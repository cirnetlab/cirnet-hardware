# Dựng và khởi động

[English](../../en/guides/getting-started.md)

Đặt cirnet-contracts-next bên cạnh checkout này và chạy cargo build --locked --bin cirnet-hardware để dựng binary Rust. Dựng worker C++ từ cirnet-simulator-next. Đặt CIRNET_CONTRACTS_ROOT thành thư mục gốc Contracts Next, CIRNET_SIM_CPP_WORKER thành đường dẫn tuyệt đối của worker và CIRNET_SIM_CPP_WORKER_SHA256 thành mã SHA-256 của worker.

Chạy cirnet-hardware --listen 127.0.0.1:50051. Trên Unix có thể dùng socket cục bộ. Cấu hình Remote dùng cùng điểm cuối cục bộ. Xem [kiểm chứng](../contributing/verification.md) để chạy bộ hồi quy.
