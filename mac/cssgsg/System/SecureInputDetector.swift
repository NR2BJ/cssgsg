// NRIME(github.com/NR2BJ/NRIME)에서 가져왔다. 거기서 여러 번 고쳐 가며 검증된 코드라 되도록 그대로 둔다.
import AppKit
import Carbon
import IOKit

final class SecureInputDetector {
    /// Returns true if the system is in Secure Input mode (e.g., password fields).
    /// Uses Carbon's IsSecureEventInputEnabled() for global detection.
    func isSecureInputActive() -> Bool {
        return IsSecureEventInputEnabled()
    }

    /// Bundle ID of the process that turned secure input on, if it can be
    /// identified.
    ///
    /// The flag itself is process-global and says nothing about who set it or
    /// why. Apps can and do leave it on for hours — a real case had a chat app
    /// holding it all day — so treating "flag is on" as "a password field is
    /// focused right here" locks the user out of composing everywhere.
    func secureInputHolderPID() -> pid_t? {
        let root = IORegistryGetRootEntry(kIOMainPortDefault)
        guard root != 0 else { return nil }
        defer { IOObjectRelease(root) }

        guard let property = IORegistryEntrySearchCFProperty(
            root,
            kIOServicePlane,
            "IOConsoleUsers" as CFString,
            kCFAllocatorDefault,
            IOOptionBits(kIORegistryIterateRecursively)
        ), let sessions = property as? [[String: Any]] else {
            return nil
        }

        for session in sessions {
            if let pid = session["kCGSSessionSecureInputPID"] as? pid_t, pid != 0 {
                return pid
            }
        }
        return nil
    }

    func secureInputHolderBundleID() -> String? {
        guard let pid = secureInputHolderPID() else { return nil }
        return NSRunningApplication(processIdentifier: pid)?.bundleIdentifier
    }

    /// Whether the process that claimed secure input is still running.
    /// macOS keeps the claim registered even after that process dies, and the
    /// flag then stays on until logout — observed in the wild.
    static func processIsAlive(_ pid: pid_t) -> Bool {
        if kill(pid, 0) == 0 { return true }
        return errno == EPERM // exists, we just may not signal it
    }

    /// One reading of the secure-input state.
    ///
    /// Composition gating and input-source recovery both need to reason about
    /// secure input, but they are not the same question — refusing to compose
    /// is cheap, forcibly selecting an input source during someone's password
    /// entry is not. They take their answers from the same snapshot so the two
    /// cannot drift apart, but each applies its own rule to it.
    struct Status {
        let isActive: Bool
        let holderIsLive: Bool
        let holderIsAuthenticationUI: Bool
        let holderIsFrontmost: Bool
        let holderIsIdentifiable: Bool

        /// Composition must not run: a live claimant plausibly owns the field
        /// being typed into. An unidentifiable holder is treated as one.
        var suppressesComposition: Bool {
            guard isActive else { return false }
            guard holderIsIdentifiable else { return true }
            guard holderIsLive else { return false }
            return holderIsAuthenticationUI || holderIsFrontmost
        }

        /// Recovery must stand down. Deliberately broader than composition
        /// gating: any live claim is someone's secure session, wherever it is,
        /// and yanking the input source out from under it is worse than simply
        /// not composing. A claim left behind by a dead process is not.
        var blocksInputSourceRecovery: Bool {
            isActive && (!holderIsIdentifiable || holderIsLive)
        }
    }

    func currentStatus() -> Status {
        let active = isSecureInputActive()
        guard active else {
            return Status(isActive: false, holderIsLive: false,
                          holderIsAuthenticationUI: false, holderIsFrontmost: false,
                          holderIsIdentifiable: true)
        }
        guard let pid = secureInputHolderPID() else {
            return Status(isActive: true, holderIsLive: false,
                          holderIsAuthenticationUI: false, holderIsFrontmost: false,
                          holderIsIdentifiable: false)
        }
        let live = Self.processIsAlive(pid)
        let bundleID = NSRunningApplication(processIdentifier: pid)?.bundleIdentifier
        let frontmost = NSWorkspace.shared.frontmostApplication?.bundleIdentifier
        return Status(
            isActive: true,
            holderIsLive: live,
            holderIsAuthenticationUI: bundleID.map { Self.authenticationBundleIDs.contains($0) } ?? false,
            holderIsFrontmost: bundleID != nil && bundleID == frontmost,
            holderIsIdentifiable: true
        )
    }

    /// Whether input-source recovery should stand down right now.
    func blocksInputSourceRecovery() -> Bool {
        currentStatus().blocksInputSourceRecovery
    }

    /// Whether composition must be suppressed for this keystroke.
    func shouldSuppressComposition() -> Bool {
        currentStatus().suppressesComposition
    }

    /// Build a Status from an already-known reading, for testing. The rule
    /// itself lives only in `Status` so the two consumers cannot diverge.
    static func shouldSuppressComposition(holderPID: pid_t?,
                                          holderIsAlive: Bool,
                                          holderBundleID: String?,
                                          frontmostBundleID: String?) -> Bool {
        Status(
            isActive: true,
            holderIsLive: holderIsAlive,
            holderIsAuthenticationUI: holderBundleID.map { authenticationBundleIDs.contains($0) } ?? false,
            holderIsFrontmost: holderBundleID != nil && holderBundleID == frontmostBundleID,
            holderIsIdentifiable: holderPID != nil
        ).suppressesComposition
    }

    /// Whether secure input is held by the system authentication UI, which is
    /// the only case where stepping the input source aside is warranted.
    func secureInputHeldByAuthenticationUI() -> Bool {
        let status = currentStatus()
        return status.isActive && status.holderIsLive && status.holderIsAuthenticationUI
    }

    /// Bundle IDs of the system authentication UI.
    ///
    /// The secure-input flag can lag the panel actually appearing — measured at
    /// ~2s on this machine — and during that gap the input method would happily
    /// compose into a field that silently drops IME insertions, so the keystroke
    /// disappears. These clients never accept composition, so identify them
    /// directly and pass every key through regardless of the flag.
    private static let authenticationBundleIDs: Set<String> = [
        "com.apple.SecurityAgent",
        "com.apple.loginwindow",
    ]

    /// Whether this client is system authentication UI (admin password prompt,
    /// login window) that must never receive composed text.
    func isAuthenticationClient(_ bundleID: String?) -> Bool {
        guard let bundleID else { return false }
        return Self.authenticationBundleIDs.contains(bundleID)
    }
}
