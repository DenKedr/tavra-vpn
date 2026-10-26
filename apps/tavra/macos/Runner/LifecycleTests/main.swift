import Cocoa
import FlutterMacOS

final class TestDelegate: AppDelegate {
  var hide = true
  override func shouldHideOnClose() -> Bool { hide }
}
func check(_ value: Bool, _ message: String) {
  guard value else { fatalError(message) }
}
let app = NSApplication.shared
let owner = TestDelegate()
app.delegate = owner
let window = MainFlutterWindow(contentRect: NSRect(x: 0, y: 0, width: 300, height: 200),
                               styleMask: [.titled, .closable], backing: .buffered, defer: false)
window.isReleasedWhenClosed = false
let controller = NSViewController()
controller.view = NSView()
window.contentViewController = controller
window.makeKeyAndOrderFront(nil)
window.close() // Path used directly by accessibility/system callers.
check(!window.isVisible, "direct close must hide")
check(window.contentViewController === controller, "content/engine owner must survive")
check(!owner.applicationShouldTerminateAfterLastWindowClosed(app), "background must survive last-window callback")
window.makeKeyAndOrderFront(nil)
check(window.isVisible, "same window reopens")
window.performClose(nil) // Menu and keyboard close path.
check(!window.isVisible, "performClose must hide")
check(window.contentViewController === controller, "keyboard close retains content")
owner.hide = false
check(owner.applicationShouldTerminateAfterLastWindowClosed(app), "disabled background permits exit")
window.makeKeyAndOrderFront(nil)
window.close()
check(!window.isVisible, "disabled close uses normal close")
print("PASS: 8 native AppKit lifecycle assertions; no preferences written; no Flutter engine launched")
