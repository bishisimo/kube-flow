//! MCP：策略、门控、stdio Server、App Gateway、本机 HTTP 服务。

pub mod audit;
pub mod capabilities;
pub mod gate;
pub mod gateway;
pub mod http_server;
pub mod ipc_client;
pub mod policy;
pub mod protocol;
pub mod read_executor;
pub mod server;
pub mod session_grant;
pub mod token;
pub mod write_executor;

pub use gateway::McpGatewayState;
pub use http_server::McpHttpState;
pub use policy::McpPolicy;
pub use server::run_stdio_blocking;
