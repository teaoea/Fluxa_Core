// 验证 Swift 调用真实 Rust 静态库，测试不启动网络或使用节点凭据。
import Foundation

precondition(fluxa_engine_abi_version() == 2)
precondition(fluxa_engine_capabilities() & 1 == 1)
for _ in 0 ..< 1000 {
  let handle = fluxa_engine_create()
  precondition(handle != 0)
  precondition(fluxa_engine_start(handle) == 1002)
  precondition(fluxa_engine_destroy(handle) == 0)
  precondition(fluxa_engine_destroy(handle) == 1201)
  precondition(fluxa_engine_start(handle) == 1201)
}

print("Swift → Rust C ABI 生命周期：1000 次创建/释放通过")
