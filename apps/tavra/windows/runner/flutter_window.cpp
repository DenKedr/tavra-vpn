#include "flutter_window.h"

#include <optional>
#include <flutter/standard_method_codec.h>
#include "resource.h"

namespace {
constexpr UINT kTrayMessage = WM_APP + 42;
constexpr UINT kOpen = 41001;
constexpr UINT kQuit = 41002;
constexpr wchar_t kSettingsKey[] = L"Software\\TAVRA\\dev.tavra.tavra";
}


#include "flutter/generated_plugin_registrant.h"

FlutterWindow::FlutterWindow(const flutter::DartProject& project)
    : project_(project) {}

FlutterWindow::~FlutterWindow() {}

bool FlutterWindow::OnCreate() {
  if (!Win32Window::OnCreate()) {
    return false;
  }

  RECT frame = GetClientArea();

  // The size here must match the window dimensions to avoid unnecessary surface
  // creation / destruction in the startup path.
  flutter_controller_ = std::make_unique<flutter::FlutterViewController>(
      frame.right - frame.left, frame.bottom - frame.top, project_);
  // Ensure that basic setup of the controller was successful.
  if (!flutter_controller_->engine() || !flutter_controller_->view()) {
    return false;
  }
  RegisterPlugins(flutter_controller_->engine());
  SetChildContent(flutter_controller_->view()->GetNativeWindow());
  DWORD stored = 1;
  DWORD size = sizeof(stored);
  if (RegGetValueW(HKEY_CURRENT_USER, kSettingsKey, L"BackgroundMode",
                   RRF_RT_REG_DWORD, nullptr, &stored, &size) == ERROR_SUCCESS) {
    background_mode_ = stored != 0;
  }
  taskbar_created_ = RegisterWindowMessageW(L"TaskbarCreated");
  RegisterTray();
  lifecycle_channel_ = std::make_unique<flutter::MethodChannel<flutter::EncodableValue>>(
      flutter_controller_->engine()->messenger(), "dev.tavra/lifecycle",
      &flutter::StandardMethodCodec::GetInstance());
  lifecycle_channel_->SetMethodCallHandler([this](const auto& call, auto result) {
    if (call.method_name() == "getBackgroundMode") {
      result->Success(flutter::EncodableValue(background_mode_));
    } else if (call.method_name() == "setBackgroundMode") {
      const auto* value = call.arguments() ? std::get_if<bool>(call.arguments()) : nullptr;
      if (!value) { result->Error("invalid_argument", "Expected bool"); return; }
      DWORD stored_value = *value ? 1 : 0;
      LSTATUS status = RegSetKeyValueW(HKEY_CURRENT_USER, kSettingsKey,
          L"BackgroundMode", REG_DWORD, &stored_value, sizeof(stored_value));
      if (status != ERROR_SUCCESS) { result->Error("storage_failed", "Cannot save background mode"); return; }
      background_mode_ = *value;
      result->Success(flutter::EncodableValue(background_mode_));
    } else { result->NotImplemented(); }
  });

  flutter_controller_->engine()->SetNextFrameCallback([&]() {
    this->Show();
  });

  // Flutter can complete the first frame before the "show window" callback is
  // registered. The following call ensures a frame is pending to ensure the
  // window is shown. It is a no-op if the first frame hasn't completed yet.
  flutter_controller_->ForceRedraw();

  return true;
}

void FlutterWindow::OnDestroy() {
  RemoveTray();
  lifecycle_channel_.reset();
  if (flutter_controller_) {
    flutter_controller_ = nullptr;
  }

  Win32Window::OnDestroy();
}

LRESULT
FlutterWindow::MessageHandler(HWND hwnd, UINT const message,
                              WPARAM const wparam,
                              LPARAM const lparam) noexcept {
  if (taskbar_created_ != 0 && message == taskbar_created_) {
    tray_registered_ = false;
    if (!RegisterTray()) Reopen(); // Never leave an inaccessible hidden window.
    return 0;
  }
  if (message == WM_CLOSE && background_mode_ && tray_registered_) {
    ShowWindow(hwnd, SW_HIDE);
    return 0;
  }
  if (message == kTrayMessage) {
    if (lparam == WM_LBUTTONUP || lparam == WM_LBUTTONDBLCLK) Reopen();
    if (lparam == WM_RBUTTONUP || lparam == WM_CONTEXTMENU) ShowTrayMenu();
    return 0;
  }
  // Give Flutter, including plugins, an opportunity to handle window messages.
  if (flutter_controller_) {
    std::optional<LRESULT> result =
        flutter_controller_->HandleTopLevelWindowProc(hwnd, message, wparam,
                                                      lparam);
    if (result) {
      return *result;
    }
  }

  switch (message) {
    case WM_FONTCHANGE:
      flutter_controller_->engine()->ReloadSystemFonts();
      break;
  }

  return Win32Window::MessageHandler(hwnd, message, wparam, lparam);
}

bool FlutterWindow::RegisterTray() {
  if (tray_registered_) return true;
  tray_ = {};
  tray_.cbSize = sizeof(tray_);
  tray_.hWnd = GetHandle();
  tray_.uID = 1;
  tray_.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP;
  tray_.uCallbackMessage = kTrayMessage;
  tray_.hIcon = LoadIcon(GetModuleHandle(nullptr), MAKEINTRESOURCE(IDI_APP_ICON));
  if (!tray_.hIcon) tray_.hIcon = LoadIcon(nullptr, IDI_APPLICATION);
  wcscpy_s(tray_.szTip, L"TAVRA");
  tray_registered_ = Shell_NotifyIconW(NIM_ADD, &tray_) != FALSE;
  return tray_registered_;
}

void FlutterWindow::RemoveTray() {
  if (tray_registered_) Shell_NotifyIconW(NIM_DELETE, &tray_);
  tray_registered_ = false;
}

void FlutterWindow::Reopen() {
  ShowWindow(GetHandle(), SW_RESTORE);
  SetForegroundWindow(GetHandle());
}

void FlutterWindow::ShowTrayMenu() {
  HMENU menu = CreatePopupMenu();
  if (!menu) return;
  AppendMenuW(menu, MF_STRING, kOpen, L"Открыть TAVRA");
  AppendMenuW(menu, MF_SEPARATOR, 0, nullptr);
  AppendMenuW(menu, MF_STRING, kQuit, L"Выйти из TAVRA");
  POINT point{};
  GetCursorPos(&point);
  SetForegroundWindow(GetHandle());
  const UINT command = TrackPopupMenu(menu, TPM_RETURNCMD | TPM_NONOTIFY | TPM_RIGHTBUTTON,
                                     point.x, point.y, 0, GetHandle(), nullptr);
  DestroyMenu(menu);
  PostMessage(GetHandle(), WM_NULL, 0, 0);
  if (command == kOpen) Reopen();
  if (command == kQuit) {
    RemoveTray();
    SetQuitOnClose(true);
    Destroy(); // Explicit quit bypasses WM_CLOSE background interception.
  }
}
