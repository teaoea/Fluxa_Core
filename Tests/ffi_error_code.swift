import Foundation
import libfluxa_core

/// 独立以固定协议值断言，不从被测 Rust 实现动态生成预期。
@main
struct ErrorCodeFFICheck {
    static func main() throws {
        let handle = fluxa_engine_create()
        precondition(handle != 0)
        defer { _ = fluxa_engine_destroy(handle) }
        let request = Data(#"{"operation":"tun_start"}"#.utf8)
        let legacy = try command(handle: handle, request: request, structured: false)
        precondition(legacy["success"] as? Bool == false)
        precondition((legacy["error_info"] as? [String: Any])?["code"] as? Int == 1002)
        let current = try command(handle: handle, request: request, structured: true)
        guard let errorInfo = current["error_info"] as? [String: Any],
              let code = errorInfo["code"] as? Int
        else { preconditionFailure("缺少整数结构化错误") }
        precondition(code == 1002 && current["error"] == nil)
        // 旧协议先验证句柄再解析 JSON；同一非法输入在不同句柄下必须准确分类。
        let malformed = Data("not-json".utf8)
        for (target, expected) in [(UInt64(0), UInt16(1201)), (handle, UInt16(1001))] {
            let response = malformed.withUnsafeBytes {
                fluxa_engine_command(target, $0.bindMemory(to: UInt8.self).baseAddress, $0.count)
            }
            precondition(response != 0)
            precondition(fluxa_engine_last_error_code() == expected)
            precondition(fluxa_engine_destroy(response) == 0)
        }
        // 资源预算不足时没有响应句柄，最低开销查询仍可用。
        var responses = [UInt64]()
        for _ in 0 ..< 63 {
            let response = request.withUnsafeBytes {
                fluxa_engine_command(handle, $0.bindMemory(to: UInt8.self).baseAddress, $0.count)
            }
            precondition(response != 0)
            responses.append(response)
        }
        let denied = request.withUnsafeBytes {
            fluxa_engine_command_v2(handle, $0.bindMemory(to: UInt8.self).baseAddress, $0.count)
        }
        precondition(denied == 0 && fluxa_engine_last_error_code() == 8001)
        var untouched = [UInt8](repeating: 0xAB, count: 1)
        let required = untouched.withUnsafeMutableBufferPointer {
            fluxa_engine_last_error_copy($0.baseAddress, $0.count)
        }
        precondition(required > 1 && untouched == [0xAB])
        precondition(fluxa_engine_last_error_code() == 8001)
        for response in responses {
            precondition(fluxa_engine_destroy(response) == 0)
        }
        precondition(fluxa_engine_last_error_code() == 0)
        print("真实 Swift FFI：全部命令统一分段整数错误、配额耗尽仍可查询、成功清除旧错误通过。")
    }

    static func command(
        handle: UInt64, request: Data, structured: Bool
    ) throws -> [String: Any] {
        let response = request.withUnsafeBytes {
            let input = $0.bindMemory(to: UInt8.self)
            return structured
                ? fluxa_engine_command_v2(handle, input.baseAddress, input.count)
                : fluxa_engine_command(handle, input.baseAddress, input.count)
        }
        precondition(response != 0)
        defer { _ = fluxa_engine_destroy(response) }
        let length = fluxa_engine_buffer_length(response)
        precondition(length > 0 && length < 4096)
        var bytes = Data(count: length)
        let copied = bytes.withUnsafeMutableBytes {
            fluxa_engine_buffer_copy(response, $0.bindMemory(to: UInt8.self).baseAddress, $0.count)
        }
        precondition(copied == length)
        guard let reply = try JSONSerialization.jsonObject(with: bytes) as? [String: Any]
        else { preconditionFailure("错误响应不是对象") }
        return reply
    }
}
