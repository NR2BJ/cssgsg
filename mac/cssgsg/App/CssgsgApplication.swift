import Cocoa

final class CssgsgApplication: NSApplication {
    let appDelegate = AppDelegate()

    override init() {
        super.init()
        delegate = appDelegate
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }
}
