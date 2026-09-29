// NRIME(github.com/NR2BJ/NRIME)의 Shared/SemanticVersion.swift를 그대로 가져왔다.
import Foundation

/// A version string with optional prerelease suffix, e.g. "1.0.9" or "1.0.9-beta.2".
///
/// Ordering follows semver §11: for the same core version a release outranks a
/// prerelease (1.0.9 > 1.0.9-beta.2), and prerelease identifiers are compared
/// left to right with numeric identifiers ordered numerically.
///
/// This is what lets the beta channel offer 1.0.9-beta.2 over 1.0.9-beta.1 while
/// still handing beta users the 1.0.9 release once it ships.
struct SemanticVersion: Comparable, Equatable, CustomStringConvertible {

    /// Numeric components (major, minor, patch, …). Missing components compare as 0.
    let core: [Int]

    /// Dot-separated prerelease identifiers ("beta", "2"). Empty for a final release.
    let prerelease: [String]

    var isPrerelease: Bool { !prerelease.isEmpty }

    var description: String {
        let base = core.map(String.init).joined(separator: ".")
        return prerelease.isEmpty ? base : base + "-" + prerelease.joined(separator: ".")
    }

    // MARK: - Parsing

    /// Parse a version string. Accepts an optional leading "v" and ignores
    /// semver build metadata ("+001"). Returns nil when no numeric core is found.
    init?(_ raw: String) {
        var text = raw.trimmingCharacters(in: .whitespacesAndNewlines)
        if text.hasPrefix("v") || text.hasPrefix("V") {
            text.removeFirst()
        }
        // Build metadata does not participate in ordering — drop it.
        if let plus = text.firstIndex(of: "+") {
            text = String(text[text.startIndex..<plus])
        }

        let coreText: String
        let prereleaseText: String
        if let dash = text.firstIndex(of: "-") {
            coreText = String(text[text.startIndex..<dash])
            prereleaseText = String(text[text.index(after: dash)...])
        } else {
            coreText = text
            prereleaseText = ""
        }

        let parsedCore = coreText.split(separator: ".").compactMap { Int($0) }
        guard !parsedCore.isEmpty else { return nil }

        self.core = parsedCore
        self.prerelease = prereleaseText.isEmpty
            ? []
            : prereleaseText.split(separator: ".").map(String.init)
    }

    // MARK: - Equatable

    /// Compares on padded core components so "1.0" equals "1.0.0", matching how
    /// `<` orders them. Synthesized equality would disagree and break Comparable.
    static func == (lhs: SemanticVersion, rhs: SemanticVersion) -> Bool {
        let count = max(lhs.core.count, rhs.core.count)
        for index in 0..<count {
            let left = index < lhs.core.count ? lhs.core[index] : 0
            let right = index < rhs.core.count ? rhs.core[index] : 0
            if left != right { return false }
        }
        return lhs.prerelease == rhs.prerelease
    }

    // MARK: - Comparable

    static func < (lhs: SemanticVersion, rhs: SemanticVersion) -> Bool {
        let count = max(lhs.core.count, rhs.core.count)
        for index in 0..<count {
            let left = index < lhs.core.count ? lhs.core[index] : 0
            let right = index < rhs.core.count ? rhs.core[index] : 0
            if left != right { return left < right }
        }

        // Same core version: a prerelease sorts below the final release.
        switch (lhs.isPrerelease, rhs.isPrerelease) {
        case (false, false): return false
        case (true, false):  return true
        case (false, true):  return false
        case (true, true):   return comparePrerelease(lhs.prerelease, rhs.prerelease)
        }
    }

    /// Compare prerelease identifier lists left to right.
    /// Numeric identifiers compare numerically and rank below alphanumeric ones;
    /// when every shared identifier matches, the shorter list ranks lower.
    private static func comparePrerelease(_ lhs: [String], _ rhs: [String]) -> Bool {
        let count = min(lhs.count, rhs.count)
        for index in 0..<count {
            let left = lhs[index]
            let right = rhs[index]
            if left == right { continue }

            switch (Int(left), Int(right)) {
            case let (leftNumber?, rightNumber?):
                return leftNumber < rightNumber
            case (_?, nil):
                return true   // numeric identifiers have lower precedence
            case (nil, _?):
                return false
            case (nil, nil):
                return left < right
            }
        }
        return lhs.count < rhs.count
    }

    // MARK: - Convenience

    /// Whether `remote` is a newer version than `current`.
    /// Unparsable input is treated as "no update" so a malformed tag never
    /// prompts a download.
    static func isNewer(remote: String, than current: String) -> Bool {
        guard let remoteVersion = SemanticVersion(remote),
              let currentVersion = SemanticVersion(current) else {
            return false
        }
        return remoteVersion > currentVersion
    }
}
