# Kiểm chứng Hardware

[English](../../en/contributing/verification.md)

Workflow Windows x64 chạy khi push lên `master` với commit chính xác của Contracts Next và Simulator Next được ghim trong workflow. Có thể chọn lại cả hai commit khi chạy thủ công. CI dựng worker và chạy bộ kiểm thử tương thích Rust cùng bộ kiểm thử vòng đời RPC của Hardware.

Tạo Actions secret `CIRNET_DEPENDENCIES_READ_TOKEN` trong [cài đặt tổ chức cirnetlab](https://github.com/organizations/cirnetlab/settings/secrets/actions). Chọn **Selected repositories** và thêm các repo chạy workflow. Dùng fine-grained token có resource owner là `cirnetlab` và quyền `Contents: read` trên các repo private được liệt kê bên dưới. Hardware cần `cirnet-contracts-next` và `cirnet-simulator-next`.

Dùng toolchain Rust đã ghim. Cấp phiên thuộc quyền sở hữu bên ngoài bằng công cụ phân bổ artifact của Contracts Next. Đặt CARGO_TARGET_DIR trong phiên và ghi tệp tạm cùng nhật ký tại đó. Chạy cargo fmt --all --check và cargo test --locked.

Sau khi thiết lập [môi trường Simulator](../guides/getting-started.md), chạy `cargo test --locked --test simulator_parity -- --ignored`. Bộ kiểm thử dùng worker đã chọn và tài nguyên chuẩn của Contracts, đồng thời kiểm tra việc từ chối mã băm sai. Bộ Rust kiểm tra hủy công việc, rào chắn generation, dừng worker, xác nhận an toàn và dữ liệu NCAP không hợp lệ. Bộ `tests/rpc` kiểm tra kế hoạch, dữ liệu thu và lệnh Stop chỉ áp dụng cho thí nghiệm tương ứng. Bộ kiểm thử dùng fixture wire và NCAP đã lưu trong kho từ lần kiểm chứng tham chiếu. Python chỉ dùng để kiểm chứng.

Để chạy bộ kiểm thử RPC, dùng Python 3.14 trong môi trường ảo bên ngoài. Cài các wheel bên thứ ba bằng `python -m pip install --only-binary=:all: --require-hashes -r requirements-verification.lock`. Khi chạy ngoại tuyến, thêm `--no-index --find-links <wheelhouse>`. Sinh binding kiểm thử từ schema Contracts Next đã chọn bằng trình biên dịch Protobuf theo nền tảng trong gói `protoc-bin-vendored` đã khóa bằng Cargo.

```powershell
$proto = Join-Path $env:CIRNET_CONTRACTS_ROOT 'contracts/hardware/v2'
$bindings = Join-Path $env:CIRNET_ARTIFACTS_SESSION 'rpc-bindings'
New-Item -ItemType Directory -Path $bindings
& $protoc --proto_path=$proto --python_out=$bindings (Join-Path $proto 'rpc_v2.proto')
$env:PYTHONPATH = $bindings
$env:CIRNET_NATIVE_HARDWARE = $hardware
$env:CIRNET_NATIVE_HARDWARE_SHA256 = $hardwareSha256
python -B -m pytest -p no:cacheprovider --basetemp (Join-Path $env:CIRNET_ARTIFACTS_SESSION 'rpc-pytest') tests/rpc
```

Đặt `$hardware` thành đường dẫn tuyệt đối của tệp thực thi đã chọn và `$hardwareSha256` thành mã băm đã rà soát. Chọn cả worker Simulator và mã băm của worker. Đặt `CIRNET_WINDOWS_PREPARED_PREVIEW=1` khi tiến trình Hardware trên Windows dùng worker riêng cho prepared Preview. Đặt `CIRNET_NATIVE_STOP_DIAGNOSTIC=1` hoặc `CIRNET_NATIVE_RPC_DIAGNOSTIC=1` để đo Stop khi worker đang bận hoặc thông lượng truy vấn khả năng. Lưu biên nhận mới tách biệt với [kết quả đo trước đây](../../../tests/rpc/fixtures/README.md).

Ghi nhận đầu vào chính xác và kết quả. Hồi quy phần mềm không xác lập năng lực vật lý. Thực thi vật lý và ngspice vẫn ở trạng thái UNAVAILABLE. Remote Next chọn ứng viên hệ thống và ghi mã băm artifact thành phần.
