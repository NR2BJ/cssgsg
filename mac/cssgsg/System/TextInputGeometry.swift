// NRIME(github.com/NR2BJ/NRIME)에서 가져왔다. 거기서 여러 번 고쳐 가며 검증된 코드라 되도록 그대로 둔다.
import Cocoa
import InputMethodKit

enum TextInputGeometry {

    /// Describes the confidence level of a caret position result.
    enum CaretSource {
        /// `firstRect` returned a precise, narrow rect.
        case precise
        /// `attributes(forCharacterIndex: caretIndex)` was used.
        case attributesAtCaret
        /// `attributes(forCharacterIndex: 0)` — only Y/height are reliable; X is the line start.
        case attributesAtZero
        /// Accessibility API (AXUIElement) provided the caret bounds.
        case accessibility
    }

    struct CaretResult {
        let rect: NSRect
        let source: CaretSource
    }

    /// Memorize last good caret position to prevent jumping to (0,0) on failure.
    /// (Inspired by fcitx5-macos coordinate memorization strategy.)
    ///
    /// Scoped to the client it came from and to a short window: a remembered
    /// position is only a better guess than nothing while it still describes
    /// the same field. Unscoped, a lookup that fails in a newly focused field
    /// would place the candidate window over the previous one's caret, which is
    /// worse than admitting the position is unknown.
    private static var lastGoodResult: CaretResult?
    private static var lastGoodClientID: String?
    private static var lastGoodTimestamp: Date?
    /// How long a remembered position stays usable.
    private static let lastGoodLifetime: TimeInterval = 10

    /// Public read-only access for InlineIndicator's attributesAtZero X fallback.
    static var lastGoodCaretRect: CaretResult? { lastGoodResult }

    /// Reset cached position (e.g., on text field switch).
    static func resetCache() {
        lastGoodResult = nil
        lastGoodClientID = nil
        lastGoodTimestamp = nil
    }

    /// Remember a position together with who it belongs to.
    private static func rememberGoodResult(_ result: CaretResult,
                                           for client: (any IMKTextInput)?) {
        lastGoodResult = result
        lastGoodClientID = client?.uniqueClientIdentifierString()
        lastGoodTimestamp = Date()
    }

    /// The remembered position, if it still describes this client and is recent.
    private static func rememberedResult(for client: (any IMKTextInput)?) -> CaretResult? {
        guard let result = lastGoodResult else { return nil }
        if let timestamp = lastGoodTimestamp,
           Date().timeIntervalSince(timestamp) > lastGoodLifetime {
            return nil
        }
        guard let clientID = lastGoodClientID else { return result }
        guard let currentID = client?.uniqueClientIdentifierString() else { return nil }
        return clientID == currentID ? result : nil
    }
    static func caretRect(for client: (any IMKTextInput)?) -> CaretResult? {
        guard let client else { return rememberedResult(for: nil) }

        // 1. Accessibility API — most accurate, works across all apps including Electron.
        //    Only called on mode switch (not per-keystroke), so 10ms overhead is acceptable.
        if let axRect = accessibilityCaretRect(), isUsableRect(axRect) {
            let result = CaretResult(rect: axRect, source: .accessibility)
            if axRect.origin.x > 1 {
                rememberGoodResult(result, for: client)
            }
            DeveloperLogger.shared.log("Geometry", "AX success", metadata: [
                "rect": String(format: "(%.0f,%.0f,%.0f,%.0f)", axRect.origin.x, axRect.origin.y, axRect.width, axRect.height),
                "cached": axRect.origin.x > 1 ? "yes" : "no(x<=1)"
            ])
            return result
        }

        // AX failed — try attributes at caret index (fcitx5-macos approach)
        DeveloperLogger.shared.log("Geometry", "AX failed, trying attributesAtCaret")

        // 2. attributes at caret index — works during composition in Firefox/native apps
        if let index = caretIndex(for: client) {
            var lineHeightRect = NSRect.zero
            client.attributes(forCharacterIndex: index, lineHeightRectangle: &lineHeightRect)
            if isUsableRect(lineHeightRect) {
                let result = CaretResult(rect: lineHeightRect, source: .attributesAtCaret)
                rememberGoodResult(result, for: client)
                DeveloperLogger.shared.log("Geometry", "attributesAtCaret success", metadata: [
                    "index": "\(index)",
                    "rect": String(format: "(%.0f,%.0f,%.0f,%.0f)", lineHeightRect.origin.x, lineHeightRect.origin.y, lineHeightRect.width, lineHeightRect.height)
                ])
                return result
            }
            DeveloperLogger.shared.log("Geometry", "attributesAtCaret failed", metadata: [
                "index": "\(index)",
                "rect": String(format: "(%.0f,%.0f,%.0f,%.0f)", lineHeightRect.origin.x, lineHeightRect.origin.y, lineHeightRect.width, lineHeightRect.height)
            ])
        }

        // 3. attributes at index 0 — simple fallback (Squirrel's approach).
        var zeroRect = NSRect.zero
        client.attributes(forCharacterIndex: 0, lineHeightRectangle: &zeroRect)
        let zeroOnScreen = NSScreen.screens.contains { $0.frame.intersects(zeroRect.insetBy(dx: -50, dy: -50)) }
        if !zeroRect.equalTo(.zero) && zeroRect.height > 0 && zeroOnScreen {
            DeveloperLogger.shared.log("Geometry", "attributesAtZero", metadata: [
                "rect": String(format: "(%.0f,%.0f,%.0f,%.0f)", zeroRect.origin.x, zeroRect.origin.y, zeroRect.width, zeroRect.height)
            ])
            return CaretResult(rect: zeroRect, source: .attributesAtZero)
        }

        DeveloperLogger.shared.log("Geometry", "All methods failed", metadata: [
            "lastGood": lastGoodResult.map { String(format: "(%.0f,%.0f)", $0.rect.origin.x, $0.rect.origin.y) } ?? "nil"
        ])
        return rememberedResult(for: client)
    }

    static func screenFrame(containing rect: NSRect) -> NSRect? {
        bestScreenFrame(for: rect, screenFrames: NSScreen.screens.map(\.visibleFrame))
    }

    static func screenFrame(containing point: NSPoint) -> NSRect? {
        bestScreenFrame(for: NSRect(origin: point, size: .zero), screenFrames: NSScreen.screens.map(\.visibleFrame))
    }

    static func panelOriginX(for anchorRect: NSRect, panelWidth: CGFloat, within screenFrame: NSRect) -> CGFloat {
        let horizontalGap: CGFloat = 2
        let preferredRightwardX = anchorRect.maxX + horizontalGap
        if preferredRightwardX + panelWidth <= screenFrame.maxX {
            return max(preferredRightwardX, screenFrame.minX)
        }

        let rightAlignedToAnchorX = anchorRect.minX - panelWidth - horizontalGap
        let clampedRightAlignedX = max(screenFrame.minX, min(rightAlignedToAnchorX, screenFrame.maxX - panelWidth))
        return clampedRightAlignedX
    }

    static func indicatorAnchorX(for rect: NSRect) -> CGFloat {
        if rect.width <= 24 {
            return rect.maxX
        }
        return rect.maxX - 10
    }

    static func caretIndex(for client: any IMKTextInput) -> Int? {
        let selectedRange = client.selectedRange()
        let markedRange = client.markedRange()

        if isPreferredSelectedRange(selectedRange, relativeTo: markedRange) {
            return max(0, selectedRange.location)
        }

        if markedRange.location != NSNotFound {
            return max(0, markedRange.location + markedRange.length)
        }

        if selectedRange.location != NSNotFound {
            return max(0, selectedRange.location)
        }

        return nil
    }

    static func bestScreenFrame(for anchorRect: NSRect, screenFrames: [NSRect]) -> NSRect? {
        guard !screenFrames.isEmpty else { return nil }

        let anchorPoint = NSPoint(x: anchorRect.midX, y: anchorRect.midY)

        if let exactMatch = screenFrames.first(where: { frame in
            frame.contains(anchorPoint) || frame.intersects(anchorRect)
        }) {
            return exactMatch
        }

        return screenFrames.min { lhs, rhs in
            squaredDistance(from: anchorPoint, to: lhs) < squaredDistance(from: anchorPoint, to: rhs)
        }
    }

    private static func squaredDistance(from point: NSPoint, to rect: NSRect) -> CGFloat {
        let dx: CGFloat
        if point.x < rect.minX {
            dx = rect.minX - point.x
        } else if point.x > rect.maxX {
            dx = point.x - rect.maxX
        } else {
            dx = 0
        }

        let dy: CGFloat
        if point.y < rect.minY {
            dy = rect.minY - point.y
        } else if point.y > rect.maxY {
            dy = point.y - rect.maxY
        } else {
            dy = 0
        }

        return (dx * dx) + (dy * dy)
    }

    // MARK: - Accessibility API

    /// Query the focused UI element's caret bounds via AXUIElement.
    /// Uses PID-direct access with 10ms timeout.
    /// Applies Input Source Pro's techniques:
    ///   - length:1 to work around macOS zero-length kAXBoundsForRange bug
    ///   - AXEnhancedUserInterface for Electron/Chromium apps
    /// Public wrapper for InlineIndicator's direct AX access.
    static func accessibilityCaretRectPublic() -> NSRect? {
        accessibilityCaretRect()
    }

    private static func accessibilityCaretRect() -> NSRect? {
        guard let frontApp = NSWorkspace.shared.frontmostApplication else { return nil }
        let pid = frontApp.processIdentifier

        let appElement = AXUIElementCreateApplication(pid)
        AXUIElementSetMessagingTimeout(appElement, 0.01) // 10ms — fast timeout, preCommitCapture handles composition

        // Activate AX on Electron/Chromium apps (they hide their AX tree by default)
        if let bundleId = frontApp.bundleIdentifier, !bundleId.hasPrefix("com.apple.") {
            AXUIElementSetAttributeValue(appElement, "AXEnhancedUserInterface" as CFString, true as CFTypeRef)
        }

        var focusedElementValue: AnyObject?
        guard AXUIElementCopyAttributeValue(appElement, kAXFocusedUIElementAttribute as CFString, &focusedElementValue) == .success else {
            return nil
        }
        let focusedElement = focusedElementValue as! AXUIElement

        // Try WebKit/Chromium-specific text markers first (Input Source Pro strategy)
        if let webRect = webAreaCaretRect(focusedElement) {
            return webRect
        }

        // Standard AX: selected text range → bounds
        var rangeValue: AnyObject?
        guard AXUIElementCopyAttributeValue(focusedElement, kAXSelectedTextRangeAttribute as CFString, &rangeValue) == .success else {
            return nil
        }

        var range = CFRange(location: 0, length: 0)
        guard AXValueGetValue(rangeValue as! AXValue, .cfRange, &range) else {
            return nil
        }

        // Use length:1 instead of length:0 to work around macOS bug
        // where kAXBoundsForRangeParameterizedAttribute returns kAXErrorNoValue for zero-length.
        var caretRange = CFRange(location: max(range.location, 0), length: 1)
        guard let caretRangeValue = AXValueCreate(.cfRange, &caretRange) else {
            return nil
        }

        var boundsValue: AnyObject?
        guard AXUIElementCopyParameterizedAttributeValue(
            focusedElement,
            kAXBoundsForRangeParameterizedAttribute as CFString,
            caretRangeValue,
            &boundsValue
        ) == .success else {
            return nil
        }

        var axBounds = CGRect.zero
        guard AXValueGetValue(boundsValue as! AXValue, .cgRect, &axBounds) else {
            return nil
        }

        return convertFromQuartz(axBounds)
    }

    /// WebKit/Chromium-specific caret detection using text markers.
    /// (Input Source Pro's findWebAreaCursor strategy)
    private static func webAreaCaretRect(_ element: AXUIElement) -> NSRect? {
        var markerRangeValue: AnyObject?
        guard AXUIElementCopyAttributeValue(element, "AXSelectedTextMarkerRange" as CFString, &markerRangeValue) == .success else {
            return nil
        }

        var boundsValue: AnyObject?
        guard AXUIElementCopyParameterizedAttributeValue(
            element,
            "AXBoundsForTextMarkerRange" as CFString,
            markerRangeValue!,
            &boundsValue
        ) == .success else {
            return nil
        }

        var axBounds = CGRect.zero
        guard AXValueGetValue(boundsValue as! AXValue, .cgRect, &axBounds) else {
            return nil
        }

        return convertFromQuartz(axBounds)
    }

    /// Convert Quartz (top-left origin) coordinates to AppKit (bottom-left origin).
    /// cssgsg: 두 좌표계는 늘 주 화면(메뉴 막대 화면, NSScreen.screens[0], AppKit 원점)을 기준으로 뒤집힌다.
    /// NRIME 코드는 x가 걸치는 화면의 높이를 써서, 높이가 다른 모니터를 쓰면 위아래가 어긋났다.
    private static func convertFromQuartz(_ quartzRect: CGRect) -> NSRect? {
        guard let primary = NSScreen.screens.first else { return nil }

        let flippedY = primary.frame.maxY - quartzRect.origin.y - quartzRect.size.height
        return NSRect(x: quartzRect.origin.x, y: flippedY, width: max(quartzRect.size.width, 1), height: quartzRect.size.height)
    }

    /// Validate the rect has positive height, isn't zero, and is within a visible screen.
    static func isUsableRect(_ rect: NSRect) -> Bool {
        guard !rect.equalTo(.zero) && rect.height > 0 else { return false }
        // Reject rects at the screen origin (0,0) — common failure mode in Firefox/Chromium
        if rect.origin.x == 0 && rect.origin.y == 0 { return false }
        // Reject rects completely outside all screens
        let onAnyScreen = NSScreen.screens.contains { screen in
            screen.frame.intersects(rect.insetBy(dx: -50, dy: -50))
        }
        return onAnyScreen
    }

    private static func isPreferredSelectedRange(_ selectedRange: NSRange, relativeTo markedRange: NSRange) -> Bool {
        guard selectedRange.location != NSNotFound else { return false }
        guard markedRange.location != NSNotFound else { return true }

        let markedEnd = markedRange.location + markedRange.length
        return selectedRange.location >= markedRange.location && selectedRange.location <= markedEnd
    }

    private static func shouldDeferSuspiciousFirstRect(_ rect: NSRect, requestedRange: NSRange, actualRange: NSRange) -> Bool {
        guard isUsableRect(rect) else { return false }
        guard rect.width > 40 else { return false }

        if requestedRange.length == 0 {
            return true
        }

        guard actualRange.location != NSNotFound, actualRange.length > 0 else {
            return false
        }

        let averageCharacterWidth = rect.width / CGFloat(actualRange.length)
        return averageCharacterWidth > 40
    }
}

private extension NSRect {
    var logDescription: String {
        String(format: "{x=%.1f,y=%.1f,w=%.1f,h=%.1f}", origin.x, origin.y, size.width, size.height)
    }
}
