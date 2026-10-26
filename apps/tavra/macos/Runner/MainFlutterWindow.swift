import Cocoa
import FlutterMacOS

class MainFlutterWindow: NSWindow {
  // AX close and direct close() do not necessarily pass through performClose.
  override func close() {
    if (NSApp.delegate as? AppDelegate)?.shouldHideOnClose() == true {
      orderOut(nil)
      return
    }
    super.close()
  }

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
    // Configure lifecycle after all nib outlets (including app delegate) are wired.
  }
}
