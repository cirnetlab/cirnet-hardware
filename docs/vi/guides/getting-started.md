# Bắt đầu với CIRNET Hardware

[English](../../en/guides/getting-started.md) · Tiếng Việt

[Mục lục tài liệu](../index.md)

## Chạy dịch vụ Python đã chọn

Trong môi trường Python mới từ 3.11 trở lên, cài các wheel đã chọn
`cirnet-hardware-service==0.1.2`, `cirnet-contracts==0.1.2` và
`cirnet-simulator==0.1.2`. Dùng đúng phiên bản mã nguồn và mã SHA-256
trong [manifest phát hành CIRNET 0.1.4](https://github.com/cirnetlab/cirnet-remote/blob/38aea6120920f48cea817827ac2277cce4bef25e/integration/release-set.json). Contracts cung cấp các
binding RPC đã sinh. Hardware chạy độc lập với mã nguồn API và các kho lân cận.

Mã nguồn hiện tại cũng dựng phiên bản `0.1.2`. Kiểm chứng bản dựng mới riêng với wheel đã chọn.

~~~sh
python -m hardware_service.server
~~~

Địa chỉ mặc định là `127.0.0.1:50051`. Dịch vụ nhận kết nối qua loopback hoặc
Unix socket. Trình duyệt thực hiện thao tác phòng thí nghiệm qua Remote API.
Tên dịch vụ RPC chuẩn là `cirnet.hardware.v2.HardwareService`. Các binding
cũng phục vụ tên `netcircuit.hardware.v2.HardwareService`. Máy khách tương
thích thử lại với tên này sau khi nhận `UNIMPLEMENTED`.

Thiết lập `CIRNET_*` được ưu tiên hơn bí danh `NETCIRCUIT_*`. Cấu hình đã
chọn dùng backend mô phỏng `legacy`. Đặt `CIRNET_SIMULATION_WORKERS` giống
nhau ở Remote và Hardware (mặc định `4`). Xem [ranh giới trạm và an toàn](../architecture/station-boundaries.md)
trước khi dùng trạm vật lý.

## Chạy ứng viên Hardware Rust

Dùng binary `cirnet-hardware` đã xác minh và worker C++ giai đoạn 1 tương ứng
từ bộ artifact ứng viên đã chọn. Đặt các giá trị sau trước khi khởi chạy.

| Thiết lập | Giá trị |
| --- | --- |
| `CIRNET_CONTRACTS_ROOT` | Đường dẫn tuyệt đối của thư mục tài nguyên chứa cây `contracts/` đã xác minh. |
| `CIRNET_SIM_CPP_WORKER` | Đường dẫn tuyệt đối của worker C++ đã chọn. |
| `CIRNET_SIM_CPP_WORKER_SHA256` | Mã SHA-256 của worker đã chọn. |
| `CIRNET_SIMULATION_BACKEND` | `cpp`. |
| `CIRNET_SIMULATION_WORKERS` | Cùng số lượng ở Remote và Hardware, từ 1 đến 64 với Hardware Rust. Mặc định là `4`. |

~~~sh
cirnet-hardware --listen 127.0.0.1:50051
~~~

Binary Rust phục vụ cả hai tên RPC 2.0, công bố khả năng thực thi số ảo và báo
trạm vật lý chưa khả dụng. Với Remote Python, đặt `CIRNET_HARDWARE_PREVIEW=true`
để dùng RPC xem trước nội bộ trong cấu hình ứng viên. Xem [cách dựng và kiểm thử Rust](../contributing/verification.md#ứng-viên-hardware-rust)
và [bằng chứng Gate 3 trên Windows](../history/stage-3-windows-verification.md) cho tổ hợp
r8-R2 đã đo. Cấu hình phát hành đã công bố dùng dịch vụ Python đã chọn.
