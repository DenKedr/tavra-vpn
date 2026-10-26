import Cocoa
import FlutterMacOS

@main
class AppDelegate: FlutterAppDelegate {
  private var statusItem: NSStatusItem?
  private var lifecycleChannel: FlutterMethodChannel?
  private weak var retainedWindow: NSWindow?
  private let backgroundKey = "backgroundMode"

  var backgroundMode: Bool {
    UserDefaults.standard.object(forKey: backgroundKey) as? Bool ?? true
  }

  override func applicationDidFinishLaunching(_ notification: Notification) {
    super.applicationDidFinishLaunching(notification)
    if let window = mainFlutterWindow,
       let controller = window.contentViewController as? FlutterViewController {
      configureLifecycle(window: window, controller: controller)
    }
  }

  func configureLifecycle(window: NSWindow, controller: FlutterViewController) {
    guard lifecycleChannel == nil else { return }
    retainedWindow = window
    window.isReleasedWhenClosed = false
    let item = NSStatusBar.system.statusItem(withLength: NSStatusItem.variableLength)
    item.button?.title = "T"
    item.button?.toolTip = "TAVRA"
    let menu = NSMenu(title: "TAVRA")
    let open = menu.addItem(withTitle: "Открыть TAVRA", action: #selector(reopenWindow), keyEquivalent: "")
    open.target = self
    menu.addItem(.separator())
    let quit = menu.addItem(withTitle: "Выйти из TAVRA", action: #selector(quitApplication), keyEquivalent: "q")
    quit.target = self
    item.menu = menu
    statusItem = item
    let channel = FlutterMethodChannel(name: "dev.tavra/lifecycle", binaryMessenger: controller.engine.binaryMessenger)
    channel.setMethodCallHandler { [weak self] call, result in
      guard let self = self else { result(FlutterError(code: "unavailable", message: "Lifecycle unavailable", details: nil)); return }
      switch call.method {
      case "getBackgroundMode": result(self.backgroundMode)
      case "setBackgroundMode":
        // NSNumber also bridges numeric values: require an actual channel boolean.
        guard let value = call.arguments as? NSNumber,
              CFGetTypeID(value) == CFBooleanGetTypeID() else {
          result(FlutterError(code: "invalid_argument", message: "Expected bool", details: nil)); return
        }
        UserDefaults.standard.set(value.boolValue, forKey: self.backgroundKey)
        result(self.backgroundMode)
      default: result(FlutterMethodNotImplemented)
      }
    }
    lifecycleChannel = channel
  }

  func shouldHideOnClose() -> Bool { backgroundMode && statusItem?.button != nil }

  @objc private func reopenWindow() {
    retainedWindow?.makeKeyAndOrderFront(nil)
    NSApp.activate(ignoringOtherApps: true)
  }

  @objc private func quitApplication() { NSApp.terminate(nil) }

  override func applicationShouldHandleReopen(_ sender: NSApplication, hasVisibleWindows flag: Bool) -> Bool {
    reopenWindow()
    return true
  }

  override func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool {
    !shouldHideOnClose()
  }

  override func applicationWillTerminate(_ notification: Notification) {
    lifecycleChannel?.setMethodCallHandler(nil)
    if let item = statusItem { NSStatusBar.system.removeStatusItem(item) }
    statusItem = nil
    super.applicationWillTerminate(notification)
  }

  override func applicationSupportsSecureRestorableState(_ app: NSApplication) -> Bool { true }
}
