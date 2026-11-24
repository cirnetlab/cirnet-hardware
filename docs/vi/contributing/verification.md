# Kiểm chứng Hardware

[English](../../en/contributing/verification.md)

Workflow Windows x64 chạy khi push lên `master` với commit chính xác của Contracts Next và Simulator Next được ghim trong workflow. Có thể chọn lại cả hai commit khi chạy thủ công. CI build worker và chạy bộ kiểm thử tương thích Hardware.

Tạo Actions secret `CIRNET_DEPENDENCIES_READ_TOKEN` trong [cài đặt tổ chức cirnetlab](https://github.com/organizations/cirnetlab/settings/secrets/actions). Chọn **Selected repositories** và thêm các repo chạy workflow. Dùng fine-grained token có resource owner là `cirnetlab` và quyền `Contents: read` trên các repo private được liệt kê bên dưới. Hardware cần `cirnet-contracts-next` và `cirnet-simulator-next`.

Dùng toolchain Rust đã ghim. Cấp phiên thuộc quyền sở hữu bên ngoài bằng công cụ phân bổ artifact của Contracts Next. Đặt CARGO_TARGET_DIR trong phiên và ghi tệp tạm cùng nhật ký tại đó. Chạy cargo fmt --all --check và cargo test --locked.

Sau khi thiết lập [môi trường Simulator](../guides/getting-started.md), chạy `cargo test --locked --test simulator_parity -- --ignored`. Bộ kiểm thử dùng worker đã chọn và tài nguyên chuẩn của Contracts, đồng thời kiểm tra việc từ chối mã băm sai. Bộ Rust kiểm tra hủy công việc, rào chắn generation, dừng worker, xác nhận an toàn và dữ liệu NCAP không hợp lệ. Bộ `tests/rpc` kiểm tra kế hoạch, dữ liệu thu và lệnh Stop chỉ áp dụng cho thí nghiệm tương ứng. Các phép đối chiếu tham chiếu cần fixture cung cấp riêng trong môi trường cách ly. Python chỉ dùng để kiểm chứng.

Để chạy bộ kiểm thử RPC trên Windows x64 hoặc Linux x64 dùng glibc, cài các wheel bên thứ ba bằng `python -m pip install --only-binary=:all: --require-hashes -r requirements-verification.lock` trong môi trường ảo bên ngoài. Chọn wheel tham chiếu Contracts và tệp thực thi Hardware native theo mã băm chính xác. Lưu kết quả pytest trong phiên artifact thuộc quyền sở hữu.

Ghi nhận đầu vào chính xác và kết quả. Hồi quy phần mềm không xác lập năng lực vật lý. Thực thi vật lý và ngspice vẫn ở trạng thái UNAVAILABLE. Remote Next chọn ứng viên hệ thống và ghi mã băm artifact thành phần.
