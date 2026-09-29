// 설정 앱(앱 업데이트)과 입력기(Mozc 엔진 업데이트)가 같이 쓰는 GitHub 릴리스 모양과 해시 확인.
import CryptoKit
import Foundation

struct GitHubRelease: Codable {
    let tagName: String
    let body: String?
    let htmlURL: String?
    let draft: Bool?
    let prerelease: Bool?
    let assets: [GitHubAsset]

    enum CodingKeys: String, CodingKey {
        case tagName = "tag_name"
        case body
        case htmlURL = "html_url"
        case draft
        case prerelease
        case assets
    }

    /// 태그 앞의 v를 뗀 버전(예: "0.1.1").
    var version: String { tagName.hasPrefix("v") ? String(tagName.dropFirst()) : tagName }

    var pkgAsset: GitHubAsset? { assets.first { $0.name.hasSuffix(".pkg") } }
}

struct GitHubAsset: Codable {
    let name: String
    let size: Int
    let browserDownloadURL: String
    /// GitHub가 계산한 내용 해시("sha256:<hex>"). pkg는 관리자 권한으로 설치하고 Mozc 엔진은 입력기가 읽으므로
    /// 받은 파일을 이것과 대조한다.
    let digest: String?

    enum CodingKeys: String, CodingKey {
        case name
        case size
        case browserDownloadURL = "browser_download_url"
        case digest
    }
}

enum GitHub {
    /// 파일의 SHA-256을 GitHub 해시("sha256:<hex>")와 비교한다. 해시가 없으면 nil, 읽지 못하면 false.
    static func fileMatchesDigest(at url: URL, expected: String?) -> Bool? {
        guard let expected, expected.lowercased().hasPrefix("sha256:") else { return nil }
        let expectedHex = String(expected.dropFirst("sha256:".count)).lowercased()
        guard let handle = try? FileHandle(forReadingFrom: url) else { return false }
        defer { try? handle.close() }
        var hasher = SHA256()
        while let chunk = try? handle.read(upToCount: 1 << 20), !chunk.isEmpty {
            hasher.update(data: chunk)
        }
        let actualHex = hasher.finalize().map { String(format: "%02x", $0) }.joined()
        return actualHex == expectedHex
    }
}
