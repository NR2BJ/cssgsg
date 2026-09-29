// NRIME(github.com/NR2BJ/NRIME)에서 가져왔다. 거기서 여러 번 고쳐 가며 검증된 코드라 되도록 그대로 둔다.
import Cocoa
import InputMethodKit

/// Custom candidate window replacing IMKCandidates.
/// Provides direct control over selection highlight, candidate list, and positioning.
final class CandidatePanel {

    /// 후보 글꼴 크기. 설정 앱이 생기면 설정에서 읽는다.
    static var fontSize: CGFloat = 14

    // MARK: - Public Properties

    /// All candidate strings (full list across all pages).
    private(set) var candidates: [String] = []

    /// cssgsg: 후보마다 같이 보일 뜻(한자 훈음 등). 없으면 빈 배열.
    /// 목록에서는 후보 옆에 흐리게, 격자에서는 고른 후보의 뜻을 아래 페이지 줄에 보인다.
    private(set) var notes: [String] = []

    /// Currently selected index in the full candidate list.
    private(set) var selectedIndex: Int = 0

    /// Number of candidates per page in list mode.
    let pageSize = 9

    /// Whether the panel is in grid (expanded) mode.
    private(set) var isGridMode: Bool = false

    /// Number of columns in grid mode.
    private let gridColumns = 5

    /// Number of rows in grid mode.
    private let gridRows = 6

    /// Grid page size (columns × rows).
    var gridPageSize: Int { gridColumns * gridRows }

    /// Effective page size depending on current mode.
    var effectivePageSize: Int { isGridMode ? gridPageSize : pageSize }

    // MARK: - Private UI

    private var panel: NSPanel?
    private var stackView: NSStackView?
    private var pageLabel: NSTextField?
    private var rowViews: [CandidateRowView] = []
    private var gridCellViews: [CandidateGridCellView] = []
    private var gridRowStacks: [NSStackView] = []

    // MARK: - Cached Layout State (avoid re-reading Settings per keystroke)

    /// Cached font size — read once per show() call from Settings.
    private var cachedFontSize: CGFloat = 14

    /// Cached max width for current candidate list in list mode.
    private var cachedListMaxWidth: CGFloat = 160

    /// Cached max cell width for current candidate list in grid mode.
    private var cachedGridCellWidth: CGFloat = 60

    /// The page that was last rendered (avoid re-rendering same page).
    private var lastRenderedPage: Int = -1

    /// The selected index when the page was last rendered (for highlight-only updates).
    private var lastRenderedSelectedIndex: Int = -1

    /// Whether last render was grid mode.
    private var lastRenderedIsGrid: Bool = false

    // MARK: - Public API

    /// Show the candidate panel with the given candidates, positioned near the caret.
    /// cssgsg: 목록/격자 모드는 엔진이 정한다(Tab은 엔진이 처리한다). candidates는 전체 후보, selectedIndex는 전체 기준.
    func show(candidates: [String], notes: [String] = [], selectedIndex: Int = 0, grid: Bool = false,
              client: (any IMKTextInput)? = nil) {
        self.candidates = candidates
        self.notes = notes.count == candidates.count ? notes : []
        self.selectedIndex = max(0, min(selectedIndex, candidates.count - 1))

        if candidates.isEmpty {
            hide()
            return
        }

        isGridMode = grid

        // Read font size ONCE per show() — avoids JSON decode on every navigation
        cachedFontSize = CandidatePanel.fontSize

        // Invalidate cached layout so next updateDisplay() does a full rebuild
        lastRenderedPage = -1
        lastRenderedSelectedIndex = -1

        // Pre-compute max widths for the entire candidate list
        cacheTextWidths()

        buildPanel()
        updateDisplay()

        if let panel = panel {
            let origin = caretOrigin(from: client, panelWidth: panel.frame.width)
            panel.setFrameOrigin(origin)
            panel.orderFront(nil)
        }
    }

    /// Hide the panel.
    func hide() {
        isGridMode = false
        panel?.orderOut(nil)
        lastRenderedPage = -1
        lastRenderedSelectedIndex = -1
    }

    /// Whether the panel is currently visible.
    func isVisible() -> Bool {
        return panel?.isVisible ?? false
    }

    /// Move selection up by 1.
    func moveUp() {
        guard !candidates.isEmpty else { return }
        if selectedIndex > 0 {
            selectedIndex -= 1
            updateDisplay()
        }
    }

    /// Move selection down by 1.
    func moveDown() {
        guard !candidates.isEmpty else { return }
        if selectedIndex < candidates.count - 1 {
            selectedIndex += 1
            updateDisplay()
        }
    }

    /// Move to the previous page.
    func pageUp() {
        guard !candidates.isEmpty else { return }
        selectedIndex = max(0, selectedIndex - effectivePageSize)
        updateDisplay()
    }

    /// Move to the next page.
    func pageDown() {
        guard !candidates.isEmpty else { return }
        selectedIndex = min(candidates.count - 1, selectedIndex + effectivePageSize)
        updateDisplay()
    }

    /// Select a candidate at a specific index.
    func select(at index: Int) {
        guard index >= 0 && index < candidates.count else { return }
        selectedIndex = index
        updateDisplay()
    }

    /// cssgsg: index번 후보의 뜻(없으면 "").
    private func note(at index: Int) -> String {
        index >= 0 && index < notes.count ? notes[index] : ""
    }

    /// Get the currently selected candidate string, or nil if empty.
    func currentSelection() -> String? {
        guard selectedIndex >= 0 && selectedIndex < candidates.count else { return nil }
        return candidates[selectedIndex]
    }

    /// Current page number (0-based).
    var currentPage: Int {
        return selectedIndex / effectivePageSize
    }

    /// Total number of pages.
    var totalPages: Int {
        return candidates.isEmpty ? 0 : ((candidates.count - 1) / effectivePageSize) + 1
    }

    // MARK: - Grid Mode

    /// Toggle grid mode on/off, repositioning the panel.
    func toggleGridMode(client: (any IMKTextInput)? = nil) {
        isGridMode = !isGridMode
        lastRenderedPage = -1 // force full rebuild on mode change
        lastRenderedSelectedIndex = -1
        updateDisplay()
        repositionPanel(client: client)
    }

    /// Exit grid mode (return to list), keeping the panel visible.
    func exitGridMode(client: (any IMKTextInput)? = nil) {
        guard isGridMode else { return }
        isGridMode = false
        lastRenderedPage = -1
        lastRenderedSelectedIndex = -1
        updateDisplay()
        repositionPanel(client: client)
    }

    /// Move selection left by 1 in grid mode.
    func moveLeft() {
        guard isGridMode, !candidates.isEmpty, selectedIndex > 0 else { return }
        selectedIndex -= 1
        updateDisplay()
    }

    /// Move selection right by 1 in grid mode.
    func moveRight() {
        guard isGridMode, !candidates.isEmpty, selectedIndex < candidates.count - 1 else { return }
        selectedIndex += 1
        updateDisplay()
    }

    /// Move selection up by one row (gridColumns) in grid mode.
    func moveUpGrid() {
        guard isGridMode, !candidates.isEmpty else { return }
        let newIndex = selectedIndex - gridColumns
        if newIndex >= 0 {
            selectedIndex = newIndex
            updateDisplay()
        }
    }

    /// Move selection down by one row (gridColumns) in grid mode.
    func moveDownGrid() {
        guard isGridMode, !candidates.isEmpty else { return }
        let newIndex = selectedIndex + gridColumns
        if newIndex < candidates.count {
            selectedIndex = newIndex
            updateDisplay()
        }
    }

    // MARK: - Private: Panel Construction

    private func buildPanel() {
        // Reuse existing panel if possible
        if panel != nil {
            return
        }

        let panel = NSPanel(
            contentRect: NSRect(x: 0, y: 0, width: 220, height: 10),
            styleMask: [.borderless, .nonactivatingPanel],
            backing: .buffered,
            defer: false
        )
        panel.level = .floating
        panel.ignoresMouseEvents = false
        panel.isOpaque = false
        panel.backgroundColor = .clear
        panel.hasShadow = true

        // Use a custom container view that updates layer colors on appearance change
        let container = AppearanceAwareContainerView()
        container.wantsLayer = true
        container.applyLayerColors()
        container.layer?.cornerRadius = 6
        container.layer?.borderWidth = 0.5
        panel.contentView = container

        // StackView and pageLabel use manual frames set in updateDisplay()
        let stack = NSStackView()
        stack.orientation = .vertical
        stack.alignment = .leading
        stack.spacing = 0

        let pageLabel = NSTextField(labelWithString: "")
        pageLabel.font = NSFont.monospacedDigitSystemFont(ofSize: 10, weight: .regular)
        pageLabel.textColor = .secondaryLabelColor
        pageLabel.alignment = .center

        container.addSubview(stack)
        container.addSubview(pageLabel)

        self.panel = panel
        self.stackView = stack
        self.pageLabel = pageLabel
    }

    // MARK: - Private: Width Caching

    /// Pre-compute max text widths for the current mode.
    /// Called once per show() instead of on every navigation.
    /// Grid widths are computed lazily when toggling to grid mode.
    private func cacheTextWidths() {
        let fontSize = cachedFontSize

        // List mode: measure all candidates to find the widest
        var listMaxWidth: CGFloat = 160
        for (i, item) in candidates.enumerated() {
            let size = CandidateRowView.rowText(item, note: note(at: i), selected: false, fontSize: fontSize).size()
            listMaxWidth = max(listMaxWidth, size.width + 50)
        }
        // 뜻이 있으면(한자) 조금 더 넓게 둔다. 넘치면 뜻 끝이 …로 잘린다.
        cachedListMaxWidth = min(listMaxWidth, notes.isEmpty ? 400 : 480)

        // Reset grid cache — will be computed on first grid display
        cachedGridCellWidth = 0
    }

    /// Compute grid cell width lazily (only when grid mode is first entered).
    private func ensureGridWidthsCached() {
        guard cachedGridCellWidth == 0 else { return }
        let gridFontSize = max(8, cachedFontSize - 1)
        let gridAttrs: [NSAttributedString.Key: Any] = [.font: NSFont.systemFont(ofSize: gridFontSize)]
        var gridMaxWidth: CGFloat = 60
        for item in candidates {
            let size = (item as NSString).size(withAttributes: gridAttrs)
            gridMaxWidth = max(gridMaxWidth, size.width + 16)
        }
        cachedGridCellWidth = min(gridMaxWidth, 120)
    }

    // MARK: - Private: Display Update

    private func updateDisplay() {
        if isGridMode {
            updateGridDisplay()
        } else {
            updateListDisplay()
        }
    }

    private func updateListDisplay() {
        guard let stackView = stackView, let pageLabel = pageLabel, let panel = panel else { return }

        let fontSize = cachedFontSize
        let numberFontSize = max(8, fontSize - 2)
        let pageFontSize = max(8, fontSize - 4)
        let rowHeight = max(24, ceil(fontSize * 1.7))
        let maxWidth = cachedListMaxWidth

        let page = currentPage
        let pageStart = page * pageSize
        let pageEnd = min(pageStart + pageSize, candidates.count)
        let pageItems = Array(candidates[pageStart..<pageEnd])
        let topPadding: CGFloat = 4
        let bottomPadding: CGFloat = 4
        let pageLabelHeight = max(16, ceil(pageFontSize * 1.6))
        let pageLabelSpacing: CGFloat = 2
        let showPageLabel = totalPages > 1
        let pageIndicatorHeight: CGFloat = showPageLabel ? (pageLabelSpacing + pageLabelHeight) : 0
        let stackHeight = CGFloat(pageItems.count) * rowHeight
        let totalHeight = topPadding + stackHeight + pageIndicatorHeight + bottomPadding

        var frame = panel.frame
        let oldHeight = frame.height
        frame.size = NSSize(width: maxWidth, height: totalHeight)
        frame.origin.y += (oldHeight - totalHeight)
        panel.setFrame(frame, display: true)

        stackView.frame = NSRect(
            x: 0,
            y: bottomPadding + pageIndicatorHeight,
            width: maxWidth,
            height: stackHeight
        )

        if showPageLabel {
            pageLabel.stringValue = "\(page + 1)/\(totalPages)"
            pageLabel.isHidden = false
            pageLabel.frame = NSRect(x: 0, y: bottomPadding,
                                     width: maxWidth, height: pageLabelHeight)
        } else {
            pageLabel.isHidden = true
        }

        // Fast path: same page, just update highlight
        if page == lastRenderedPage && !lastRenderedIsGrid && rowViews.count == pageItems.count {
            updateListHighlight(pageStart: pageStart, rowHeight: rowHeight)
            lastRenderedSelectedIndex = selectedIndex
            return
        }

        // Full rebuild needed (page change or first render)
        clearStackView()

        pageLabel.font = NSFont.monospacedDigitSystemFont(ofSize: pageFontSize, weight: .regular)

        for (i, item) in pageItems.enumerated() {
            let globalIndex = pageStart + i
            let isSelected = (globalIndex == selectedIndex)
            let number = (i + 1) % 10

            let row = CandidateRowView(
                number: number == 0 ? 0 : number,
                text: item,
                note: note(at: globalIndex),
                isSelected: isSelected,
                width: maxWidth,
                fontSize: fontSize,
                numberFontSize: numberFontSize,
                rowHeight: rowHeight
            )
            stackView.addArrangedSubview(row)
            rowViews.append(row)
        }

        lastRenderedPage = page
        lastRenderedSelectedIndex = selectedIndex
        lastRenderedIsGrid = false
    }

    /// Fast highlight-only update for list mode — just toggle background colors.
    private func updateListHighlight(pageStart: Int, rowHeight: CGFloat) {
        for (i, row) in rowViews.enumerated() {
            let globalIndex = pageStart + i
            let isSelected = (globalIndex == selectedIndex)
            row.updateHighlight(isSelected: isSelected)
        }
    }

    private func updateGridDisplay() {
        guard let stackView = stackView, let pageLabel = pageLabel, let panel = panel else { return }

        ensureGridWidthsCached()

        let fontSize = cachedFontSize
        let gridFontSize = max(8, fontSize - 1)
        let pageFontSize = max(8, fontSize - 4)
        let maxCellWidth = cachedGridCellWidth

        let page = currentPage
        let pageStart = page * gridPageSize
        let pageEnd = min(pageStart + gridPageSize, candidates.count)
        let pageItems = Array(candidates[pageStart..<pageEnd])
        let cellHeight = max(26, ceil(fontSize * 1.85))
        let sidePadding: CGFloat = 4
        let panelWidth = maxCellWidth * CGFloat(gridColumns) + sidePadding * 2
        let actualRows = (pageItems.count + gridColumns - 1) / gridColumns
        let topPadding: CGFloat = 4
        let bottomPadding: CGFloat = 4
        let pageLabelHeight = max(16, ceil(pageFontSize * 1.6))
        let pageLabelSpacing: CGFloat = 2
        // cssgsg: 격자 칸에는 뜻을 못 넣으니, 고른 후보의 뜻을 페이지 줄에 같이 보인다.
        let selectedNote = note(at: selectedIndex)
        let showPageLabel = totalPages > 1 || !selectedNote.isEmpty
        let pageIndicatorHeight: CGFloat = showPageLabel ? (pageLabelSpacing + pageLabelHeight) : 0
        let gridHeight = CGFloat(actualRows) * cellHeight
        let totalHeight = topPadding + gridHeight + pageIndicatorHeight + bottomPadding

        var frame = panel.frame
        let oldHeight = frame.height
        frame.size = NSSize(width: panelWidth, height: totalHeight)
        frame.origin.y += (oldHeight - totalHeight)
        panel.setFrame(frame, display: true)

        stackView.frame = NSRect(
            x: sidePadding,
            y: bottomPadding + pageIndicatorHeight,
            width: panelWidth - sidePadding * 2,
            height: gridHeight
        )

        if showPageLabel {
            let pages = totalPages > 1 ? "\(page + 1)/\(totalPages)" : ""
            pageLabel.stringValue = [selectedNote, pages].filter { !$0.isEmpty }.joined(separator: "   ")
            pageLabel.lineBreakMode = .byTruncatingHead
            pageLabel.isHidden = false
            pageLabel.frame = NSRect(x: 0, y: bottomPadding,
                                     width: panelWidth, height: pageLabelHeight)
        } else {
            pageLabel.isHidden = true
        }

        // Fast path: same page, just update highlight
        if page == lastRenderedPage && lastRenderedIsGrid && gridCellViews.count == pageItems.count {
            updateGridHighlight(pageStart: pageStart)
            lastRenderedSelectedIndex = selectedIndex
            return
        }

        // Full rebuild
        clearStackView()

        pageLabel.font = NSFont.monospacedDigitSystemFont(ofSize: pageFontSize, weight: .regular)

        for row in 0..<actualRows {
            let rowStack = NSStackView()
            rowStack.orientation = .horizontal
            rowStack.spacing = 0
            rowStack.alignment = .centerY

            for col in 0..<gridColumns {
                let idx = row * gridColumns + col
                guard idx < pageItems.count else { break }
                let globalIdx = pageStart + idx
                let isSelected = globalIdx == selectedIndex

                let cell = CandidateGridCellView(
                    text: pageItems[idx],
                    isSelected: isSelected,
                    width: maxCellWidth,
                    height: cellHeight,
                    fontSize: gridFontSize
                )
                rowStack.addArrangedSubview(cell)
                gridCellViews.append(cell)
            }
            stackView.addArrangedSubview(rowStack)
            gridRowStacks.append(rowStack)
        }

        lastRenderedPage = page
        lastRenderedSelectedIndex = selectedIndex
        lastRenderedIsGrid = true
    }

    /// Fast highlight-only update for grid mode — just toggle background colors.
    private func updateGridHighlight(pageStart: Int) {
        for (i, cell) in gridCellViews.enumerated() {
            let globalIdx = pageStart + i
            let isSelected = globalIdx == selectedIndex
            cell.updateHighlight(isSelected: isSelected)
        }
    }

    /// Remove all arranged subviews from the stack.
    private func clearStackView() {
        guard let stackView = stackView else { return }
        for view in rowViews {
            stackView.removeArrangedSubview(view)
            view.removeFromSuperview()
        }
        rowViews.removeAll()
        for view in gridCellViews {
            view.removeFromSuperview()
        }
        gridCellViews.removeAll()
        for rowStack in gridRowStacks {
            stackView.removeArrangedSubview(rowStack)
            rowStack.removeFromSuperview()
        }
        gridRowStacks.removeAll()
        // Also remove any remaining subviews
        for subview in stackView.arrangedSubviews {
            stackView.removeArrangedSubview(subview)
            subview.removeFromSuperview()
        }
    }

    /// Reposition the panel after a size change (e.g., grid toggle).
    private func repositionPanel(client: (any IMKTextInput)? = nil) {
        guard let panel = panel else { return }
        let origin = caretOrigin(from: client, panelWidth: panel.frame.width)
        panel.setFrameOrigin(origin)
    }

    // MARK: - Private: Positioning

    private func caretOrigin(from client: (any IMKTextInput)?, panelWidth: CGFloat) -> NSPoint {
        if let result = TextInputGeometry.caretRect(for: client) {
            let lineHeightRect = result.rect
            let panelHeight = panel?.frame.height ?? 200
            let gap: CGFloat = 2
            let belowY = lineHeightRect.origin.y - panelHeight - gap

            // For attributesAtZero fallback, X is unreliable (points to line start).
            // Keep current panel X position if visible, otherwise use the rect X as best guess.
            let x: CGFloat
            if result.source == .attributesAtZero, let panel, panel.isVisible {
                x = panel.frame.origin.x
            } else {
                x = lineHeightRect.origin.x
            }

            // Ensure panel stays on screen
            if let screenFrame = TextInputGeometry.screenFrame(containing: lineHeightRect) {
                let y: CGFloat
                let aboveY = lineHeightRect.origin.y + lineHeightRect.height + gap
                if belowY < screenFrame.minY && aboveY + panelHeight <= screenFrame.maxY {
                    y = aboveY
                } else {
                    y = max(belowY, screenFrame.minY)
                }
                // Left-align with caret position, clamped to screen bounds
                let clampedX = max(screenFrame.minX, min(x, screenFrame.maxX - panelWidth))
                return NSPoint(x: clampedX, y: y)
            }
            return NSPoint(x: x, y: belowY)
        }

        if let panel, panel.isVisible {
            return panel.frame.origin
        }

        // Fallback: near mouse
        let mouseLocation = NSEvent.mouseLocation
        if let screenFrame = TextInputGeometry.screenFrame(containing: mouseLocation) {
            DeveloperLogger.shared.log("geometry", "Candidate panel fell back to mouse anchor", metadata: [
                "mouse": String(format: "{x=%.1f,y=%.1f}", mouseLocation.x, mouseLocation.y)
            ])
            return NSPoint(
                x: max(screenFrame.minX, min(mouseLocation.x - 24, screenFrame.maxX - panelWidth)),
                y: max(screenFrame.minY, mouseLocation.y - 200)
            )
        }
        return NSPoint(x: mouseLocation.x - 24, y: mouseLocation.y - 200)
    }
}

// MARK: - CandidateRowView

/// A single row in the candidate panel: [number] [text] [note]
private class CandidateRowView: NSView {

    private let numberLabel: NSTextField
    private let textLabel: NSTextField
    /// cssgsg: 후보 뒤에 흐리게 붙이는 뜻(없으면 "").
    private let text: String
    private let note: String
    private let fontSize: CGFloat

    /// cssgsg: 후보 글자와 뜻을 한 줄로(같은 기준선). 뜻은 작고 흐리게.
    static func rowText(_ text: String, note: String, selected: Bool, fontSize: CGFloat) -> NSAttributedString {
        let s = NSMutableAttributedString(string: text, attributes: [
            .font: NSFont.systemFont(ofSize: fontSize),
            .foregroundColor: selected ? NSColor.white : NSColor.labelColor,
        ])
        if !note.isEmpty {
            s.append(NSAttributedString(string: "  " + note, attributes: [
                .font: NSFont.systemFont(ofSize: max(8, fontSize - 3)),
                .foregroundColor: selected ? NSColor(white: 1, alpha: 0.75) : NSColor.secondaryLabelColor,
            ]))
        }
        return s
    }

    init(number: Int, text: String, note: String = "", isSelected: Bool, width: CGFloat,
         fontSize: CGFloat = 14, numberFontSize: CGFloat = 12, rowHeight: CGFloat = 24) {
        let numberWidth = max(22, ceil(numberFontSize * 1.8))
        self.text = text
        self.note = note
        self.fontSize = fontSize

        numberLabel = NSTextField(labelWithString: number > 0 ? "\(number)." : "")
        numberLabel.font = NSFont.monospacedDigitSystemFont(ofSize: numberFontSize, weight: .regular)
        numberLabel.frame = NSRect(x: 6, y: 0, width: numberWidth, height: rowHeight)

        let textX = 6 + numberWidth + 2
        textLabel = NSTextField(labelWithString: text)
        textLabel.font = NSFont.systemFont(ofSize: fontSize)
        textLabel.lineBreakMode = .byTruncatingTail
        textLabel.frame = NSRect(x: textX, y: 0, width: width - textX - 6, height: rowHeight)

        super.init(frame: NSRect(x: 0, y: 0, width: width, height: rowHeight))

        wantsLayer = true
        _isSelected = isSelected
        applyColors(isSelected: isSelected)

        addSubview(numberLabel)
        addSubview(textLabel)

        // Fixed size
        translatesAutoresizingMaskIntoConstraints = false
        NSLayoutConstraint.activate([
            widthAnchor.constraint(equalToConstant: width),
            heightAnchor.constraint(equalToConstant: rowHeight),
        ])
    }

    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    /// Update only the highlight state without recreating the view.
    func updateHighlight(isSelected: Bool) {
        _isSelected = isSelected
        applyColors(isSelected: isSelected)
    }

    override func viewDidChangeEffectiveAppearance() {
        super.viewDidChangeEffectiveAppearance()
        applyColors(isSelected: _isSelected)
    }

    private var _isSelected: Bool = false

    private func applyColors(isSelected: Bool) {
        if isSelected {
            layer?.backgroundColor = NSColor.selectedContentBackgroundColor.cgColor
        } else {
            layer?.backgroundColor = NSColor.clear.cgColor
        }
        let secondaryColor: NSColor = isSelected ? .init(white: 1, alpha: 0.7) : .secondaryLabelColor
        numberLabel.textColor = secondaryColor
        textLabel.attributedStringValue = Self.rowText(text, note: note, selected: isSelected, fontSize: fontSize)
        textLabel.lineBreakMode = .byTruncatingTail
    }
}

// MARK: - CandidateGridCellView

/// A single cell in the grid-mode candidate panel.
private class CandidateGridCellView: NSView {

    private let textLabel: NSTextField

    init(text: String, isSelected: Bool, width: CGFloat, height: CGFloat, fontSize: CGFloat = 13) {
        textLabel = NSTextField(labelWithString: text)
        textLabel.font = NSFont.systemFont(ofSize: fontSize)
        textLabel.lineBreakMode = .byTruncatingTail
        textLabel.alignment = .center
        textLabel.frame = NSRect(x: 2, y: 0, width: width - 4, height: height)

        super.init(frame: NSRect(x: 0, y: 0, width: width, height: height))

        wantsLayer = true
        _isSelected = isSelected
        applyColors(isSelected: isSelected)

        addSubview(textLabel)

        translatesAutoresizingMaskIntoConstraints = false
        NSLayoutConstraint.activate([
            widthAnchor.constraint(equalToConstant: width),
            heightAnchor.constraint(equalToConstant: height),
        ])
    }

    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    /// Update only the highlight state without recreating the view.
    func updateHighlight(isSelected: Bool) {
        _isSelected = isSelected
        applyColors(isSelected: isSelected)
    }

    override func viewDidChangeEffectiveAppearance() {
        super.viewDidChangeEffectiveAppearance()
        applyColors(isSelected: _isSelected)
    }

    private var _isSelected: Bool = false

    private func applyColors(isSelected: Bool) {
        if isSelected {
            layer?.backgroundColor = NSColor.selectedContentBackgroundColor.cgColor
            layer?.cornerRadius = 3
        } else {
            layer?.backgroundColor = NSColor.clear.cgColor
            layer?.cornerRadius = 0
        }
        textLabel.textColor = isSelected ? .white : .labelColor
    }
}

// MARK: - AppearanceAwareContainerView

/// Container view that re-applies layer colors when dark/light mode changes.
private class AppearanceAwareContainerView: NSView {

    func applyLayerColors() {
        layer?.backgroundColor = NSColor.windowBackgroundColor.cgColor
        layer?.borderColor = NSColor.separatorColor.cgColor
    }

    override func viewDidChangeEffectiveAppearance() {
        super.viewDidChangeEffectiveAppearance()
        applyLayerColors()
    }
}
