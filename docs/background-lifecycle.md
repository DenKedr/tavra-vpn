# Background lifecycle — desktop implementation

Default: background mode enabled. Closing the desktop window hides it and preserves the existing Flutter engine and its process. This does not create a VPN, activate Network Extension, install login/startup tasks or guarantee a monitoring service exists. Future connection/monitor ownership must remain independent of window visibility and report actual state.

## Native contract

Method channel `dev.tavra/lifecycle`:

- `getBackgroundMode`, no arguments: returns bool.
- `setBackgroundMode`, a bool argument: persists and returns the saved bool; invalid arguments produce `invalid_argument`. Windows registry failure returns `storage_failed`.

macOS stores nonsecret `backgroundMode` in UserDefaults, bundle `dev.tavra.tavra`. Windows stores DWORD `BackgroundMode` under HKCU `Software\TAVRA\dev.tavra.tavra`. No autostart keys are written. Missing setting means true; false restores normal close/exit behavior. UI should fetch on startup and reflect native errors instead of assuming a save succeeded.

## macOS

NSStatusItem titled T with tooltip TAVRA has Open and explicit Quit menu items. Window close/⌘W hides via `orderOut` when the setting and menu-bar item are available. It does not close the NSWindow or destroy the engine. Dock reopen brings the same window forward. Quit menu/⌘Q calls normal application termination and cleans up the status item and method handler. Closing with background disabled exits normally. No LSUIElement flag: Dock remains available.

AppDelegate initialization runs in applicationDidFinishLaunching after nib outlet wiring and plugin registration. Existing generated registration stays owned by the UI integration. Product name is TAVRA. Tray glyph is a plain template letter, not an invented VPN status indicator; product icon artwork conversion is deferred.

## Windows

Shell_NotifyIcon installs the current packaged app icon and TAVRA tooltip. Click reopens; right-click opens Open/Quit menu. WM_CLOSE hides only if background mode is enabled **and** registration succeeded. Failed registration falls back to normal close so the user is never left with an inaccessible background window. TaskbarCreated re-registers the tray after Explorer restart; registration failure reopens the window. Explicit Quit removes the icon and destroys the window without sending WM_CLOSE, ending the normal message loop. OnDestroy removes the icon before destroying Flutter/channel. User logoff/shutdown retains standard system handling.

No Windows runtime/toolchain is available on this Mac. Required Windows acceptance: MSVC build, both channel methods/type rejection, registry persistence/failure, close/reopen with unchanged engine state, background-disabled close, explicit Quit, Explorer restart and shutdown. Test app icon availability in packaging. Optional Explorer failure UI and accessibility/localization polishing can follow observed results.

## Android requirement, not implemented

Window-style close semantics do not apply. A real active VPN must belong to VpnService with foreground notification and correct target-SDK permissions/type; Activity dismissal must not terminate an established service. Notification reopens UI; explicit Disconnect stops VPN according to kill-switch policy, explicit app exit requires product policy. Android process death, Doze, reboot and system always-on/block-without-VPN need independent runtime checks. No VpnService or fake connected state was added here.

## Verification

2026-10-27: `swiftc -frontend -parse` passes for both native Swift files. ARM64/macOS13 `swiftc -typecheck` with local FlutterMacOS framework passes, using a temporary no-op RegisterGeneratedPlugins declaration because generated shared_preferences_foundation module requires Flutter's full build pipeline. This verifies the native lifecycle API types, not plugins or launch. Parent coordinates full Flutter macOS build and real CUA close/menu/Dock reopen and explicit-exit acceptance. Windows source and CMake linkage prepared; actual MSVC/runtime verification remains pending.

Regression update: lifecycle setup runs in applicationDidFinishLaunching after nib outlet wiring. Both performClose and direct NSWindow.close are intercepted; the latter covers direct/accessibility close paths. Last-window termination returns false when background hiding is available. Native AppKit harness `macos/Runner/LifecycleTests/verify.sh` passes 8 assertions for direct/menu close, reopening the same window and retained content. It uses a test delegate without launching Flutter engine or changing preferences, so actual nib/menu initialization still requires full app CUA acceptance.

Actual parent CUA acceptance (2026-10-27): accessibility close button leaves TAVRA running; existing window can be raised again and renders correctly; explicit Cmd-Q terminates the process. Native process presence was checked independently before explicit quit and absence after it. Initial close regression was reproduced before the fix. Windows and Android checks above remain pending.
