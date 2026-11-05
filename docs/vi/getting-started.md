# Bắt đầu với CIRNET Hardware

[English](../en/getting-started.md) · Tiếng Việt

[Mục lục tài liệu](index.md)

Mã nguồn hiện tại dựng `cirnet-hardware-service==0.1.2` cho Python từ 3.11. Trong một môi trường mới, cài wheel Hardware cùng các wheel đã được rà soát `cirnet-contracts==0.1.2` và `cirnet-simulator==0.1.2`. Dùng đúng mã băm artifact trong danh mục phát hành hệ thống. Contracts cung cấp các binding RPC đã sinh. Có thể cài Hardware độc lập với mã nguồn API và các kho lân cận.

Khởi động dịch vụ từ môi trường đã cài các wheel.

~~~sh
python -m hardware_service.server
~~~

Địa chỉ mặc định là `127.0.0.1:50051`. Dịch vụ chỉ lắng nghe trên loopback hoặc Unix socket. Trình duyệt thực hiện thao tác phòng thí nghiệm qua Remote API. Tên dịch vụ chuẩn là `cirnet.hardware.v2.HardwareService`. Binding đã sinh cũng phục vụ `netcircuit.hardware.v2.HardwareService`, và máy khách chỉ thử tuyến này sau khi nhận `UNIMPLEMENTED`.

Thiết lập `CIRNET_*` được ưu tiên hơn bí danh `NETCIRCUIT_*`. Với phòng thí nghiệm ảo, đặt `CIRNET_SIMULATION_WORKERS` giống nhau ở API và Hardware. Giá trị mặc định là `4`. Trạm vật lý chỉ khả dụng khi có trình điều khiển và phần cứng đã được kiểm chứng. Xem [ranh giới trạm và an toàn](station-boundaries.md) trước khi diễn giải kết quả RPC là trạng thái sẵn sàng của phần cứng.

Artifact `v0.1.1` đã công bố giữ nguyên phiên bản và mã SHA256 ban đầu. Tái lập các artifact đó trong môi trường riêng. Để dựng bản mới, làm theo [hướng dẫn kiểm chứng](verification.md).
