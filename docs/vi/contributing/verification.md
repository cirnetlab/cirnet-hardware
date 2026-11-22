# Kiểm chứng Hardware

[English](../../en/contributing/verification.md)

Workflow Windows x64 chạy khi push lên `master` với commit chính xác của Contracts Next và Simulator Next được ghim trong workflow. Có thể chọn lại cả hai commit khi chạy thủ công. Cấu hình `CIRNET_DEPENDENCIES_READ_TOKEN` có quyền đọc Contents ở cả hai repo. CI build worker và chạy bộ kiểm thử tương thích Hardware.

Dùng toolchain Rust đã ghim. Cấp phiên thuộc quyền sở hữu bên ngoài bằng công cụ phân bổ artifact của Contracts Next. Đặt CARGO_TARGET_DIR trong phiên và ghi tệp tạm cùng nhật ký tại đó. Chạy cargo fmt --all --check và cargo test --locked.

Sau khi thiết lập [môi trường Simulator](../guides/getting-started.md), chạy cargo test --locked --test simulator_parity -- --ignored. Bộ kiểm thử dùng worker hiện tại và tài nguyên chuẩn của Contracts. Bộ Rust kiểm tra hủy công việc, rào chắn generation, dừng worker và xác nhận an toàn. Bộ tùy chọn tests/rpc có đối chiếu tham chiếu Python và cần fixture tham chiếu cung cấp riêng trong môi trường cách ly. Python chỉ dùng cho nhánh kiểm chứng này.

Ghi nhận đầu vào chính xác và kết quả. Hồi quy phần mềm không xác lập năng lực vật lý. Thực thi vật lý và ngspice vẫn ở trạng thái UNAVAILABLE. Remote Next chọn ứng viên hệ thống và ghi mã băm artifact thành phần.
