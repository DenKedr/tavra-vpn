import Cocoa
import FlutterMacOS

class MainFlutterWindow: NSWindow {
  override func performClose(_ sender: Any?) {
    if (NSApp.delegate as? AppDelegate)?.shouldHideOnClose() == true {
      orderOut(sender)
      return
    }
    super.performClose(sender)
  }

  override func awakeFromNib() {
    let flutterViewController = FlutterViewController()
    let windowFrame = self.frame
    self.contentViewController = flutterViewController
    self.setFrame(windowFrame, display: true)

    RegisterGeneratedPlugins(registry: flutterViewController)

    super.awakeFromNib()
    (NSApp.delegate as? AppDelegate)?.configureLifecycle(window: self, controller: flutterViewController)
  }
}
