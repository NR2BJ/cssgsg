import Foundation

// 입력기가 자기 기본값 저장소(com.cssgsg.inputmethod.app)에 적고 설정 앱이 읽는 상태. 권한과 Mozc 엔진은
// 입력기 프로세스의 것이라 설정 앱이 직접 볼 수 없다. 바뀌면 입력기가 분산 알림을 보내고, 설정 앱은 다시 읽는다.

/// 입력기의 키 보내기·손쉬운 사용(macOS 27: 기기 제어 및 데이터 접근) 권한. 입력기가 마지막으로 본 대로.
/// NRIME PermissionStatus와 같다.
struct PermissionStatus: Codable, Equatable {
    var postEvents: Bool
    var accessibility: Bool
    var checkedAt: Date

    /// 둘 중 하나면 키를 보낼 수 있다(macOS가 키 보내기를 손쉬운 사용 권한으로도 허락한다).
    var granted: Bool { postEvents || accessibility }

    static let defaultsKey = "permissionStatus"
    /// 입력기 → 설정 앱: 다시 적었다.
    static let changedNotification = Notification.Name("com.cssgsg.permission-status-changed")
}

/// 입력기가 쓰는 Mozc 엔진과, 받아 두고 다음 시작을 기다리는 새 엔진. NRIME MozcStatus와 같다.
struct MozcStatus: Codable, Equatable {
    struct Build: Codable, Equatable {
        let version: String
        /// Mozc 커밋 날짜(yyyy-MM-dd).
        let date: String
        let commit: String
        /// cssgsg 래퍼(mozc/cssgsg) 판(0.7.2). 래퍼만 고쳐 다시 빌드한 엔진은 Mozc 커밋이 같고 이것만 크다. 없으면 0.
        var wrapper: Int? = nil
    }

    var active: Build?
    /// "bundled"(앱에 든 것) 또는 "downloaded".
    var activeSource: String?
    /// 받아 두었다. 입력기가 다음에 시작할 때부터 쓴다.
    var pending: Build?
    /// 마지막으로 확인에 성공한 때(실패한 확인은 적지 않는다).
    var checkedAt: Date?
    /// 마지막 확인이 실패했으면 그때(오프라인 등). 다음에 성공하면 지운다.
    var checkFailedAt: Date?

    static let defaultsKey = "mozcStatus"
    static let changedNotification = Notification.Name("com.cssgsg.mozc-status-changed")
}

extension Cssgsg {
    /// 입력기가 적어 둔 상태 하나(JSON). 없거나 못 읽으면 nil.
    static func imeStatus<T: Decodable>(_ type: T.Type, key: String) -> T? {
        guard let data: Data = imePreference(key) else { return nil }
        let decoder = JSONDecoder()
        decoder.dateDecodingStrategy = .secondsSince1970
        return try? decoder.decode(T.self, from: data)
    }

    /// 입력기 쪽에서 상태를 적고 설정 앱에 알린다.
    static func publishIMEStatus<T: Encodable>(_ value: T, key: String, notification: Notification.Name) {
        let encoder = JSONEncoder()
        encoder.dateEncodingStrategy = .secondsSince1970
        guard let data = try? encoder.encode(value) else { return }
        UserDefaults.standard.set(data, forKey: key)
        DistributedNotificationCenter.default().postNotificationName(notification, object: nil, userInfo: nil,
                                                                      deliverImmediately: true)
    }
}
