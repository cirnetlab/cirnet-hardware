# Ranh giới trạm và an toàn

[English](../../en/architecture/station-boundaries.md) · Tiếng Việt

[Mục lục tài liệu](../index.md)

Dịch vụ 0.3.0-dev.1 triển khai Hardware RPC cục bộ và giám sát worker
Simulator C++ đã xác minh SHA-256. Dịch vụ cung cấp trạm ảo.
[Kiểm chứng](../contributing/verification.md)

| Bên sở hữu | Ranh giới |
| --- | --- |
| Hardware | Hardware RPC 2.0 chỉ dùng cục bộ, vòng đời trạm, giám sát worker Simulator, Start có bảo vệ, Stop an toàn và truyền tải FPGA nội bộ. Hardware báo lỗi dọn dẹp để Remote cách ly lease trong khi trạm vẫn bị khóa. |
| Remote | API Web, xác thực và phân quyền, lease bền vững, cách ly và lưu trữ bản thu. Remote chỉ giải phóng lease sau khi nhận xác nhận dọn dẹp an toàn hợp lệ. |
| Simulator, Contracts và FPGA | Simulator sở hữu mô hình điện và khung CNP1/CNP2 nội bộ. Contracts sở hữu RPC và schema logic chuẩn. FPGA sở hữu RTL và việc nghiệm thu bo. |

Dùng [kiến trúc Lab-First](https://github.com/cirnetlab/cirnet-remote/blob/master/docs/cirnet-architecture-lab-first-vi.md) cho an toàn hệ thống và [hợp đồng an toàn vật lý của Remote](https://github.com/cirnetlab/cirnet-remote/blob/master/docs/vi/architecture/physical-lab-safety-contract.md) cho yêu cầu giữa ứng dụng và trạm.

## Bố cục mã nguồn

| Đường dẫn | Trách nhiệm |
| --- | --- |
| [`src/main.rs`](../../../src/main.rs) | Khởi động và dừng dịch vụ cục bộ. |
| [`src/rpc.rs`](../../../src/rpc.rs), [`src/rpc/`](../../../src/rpc) | Cung cấp Hardware RPC và Preview nội bộ. Việc chuyển đổi trường yêu cầu và điều phối khi tiếp nhận Start nằm cùng trình xử lý RPC. |
| [`src/station.rs`](../../../src/station.rs), [`src/station/`](../../../src/station) | Quản lý phiên trạm, các điều kiện an toàn và việc giám sát dọn dẹp tác vụ. |
| [`src/simulator.rs`](../../../src/simulator.rs), [`src/simulator/`](../../../src/simulator) | Biên dịch kế hoạch ảo, tạo phép chiếu DC giới hạn, mã hóa khung CNP và giám sát tiến trình worker. |
| [`proto/preview.proto`](../../../proto/preview.proto), [`tests/rpc/`](../../../tests/rpc) | Định nghĩa giao diện Preview nội bộ và kiểm tra hành vi dịch vụ qua ranh giới RPC. |

## Hành vi của trạm

| Trạm | Hành vi hiện tại |
| --- | --- |
| Ảo | Mỗi thí nghiệm có phiên độc lập. Backend `cpp` dùng các worker native được giám sát và xác minh SHA-256. Backend cung cấp bốn kênh Logic Analyzer, không có kênh Scope hoặc Generator. `ngspice` báo không khả dụng. |
| Vật lý | Khi thiếu trình điều khiển, trạm báo `OFFLINE` hoặc không khả dụng, số slot và kênh bằng 0, và `hardware_applied=false`. Bộ điều hợp Serial và USB-SPI mới là chỗ dành cho tích hợp sau này. |

Với trạm vật lý đã được nghiệm thu, Start yêu cầu quyền thực thi, kết quả an toàn còn hiệu lực, kế hoạch khả thi khớp đúng trạm và thí nghiệm, cùng bằng chứng phần cứng về bảo vệ và đủ tài nguyên. Gắn điểm cuối với tô pô đã quan sát trước khi định tuyến hạ tầng. Physical Compiler giữ nguyên IC và dây do sinh viên lắp. Chỉ cấp điện có bảo vệ và thu tín hiệu khi có permit phần cứng tương ứng. An toàn, tô pô và tính đúng chức năng là các quyết định riêng. Chỉ riêng việc chưa nhận diện linh kiện hoặc tô pô không khớp chưa đủ để kết luận về an toàn điện.

Dữ liệu NCAP số đi trong `OperationReply.capture_payload`. `result_json` đi kèm chứa siêu dữ liệu. Kiểm thử phần mềm, trình điều khiển thử nghiệm và mô phỏng RTL không chứng nhận bảo vệ điện, ABI phần cứng, thu nhận tín hiệu thật hoặc thời gian đáp ứng.

Trên Windows, dịch vụ native khởi động một worker đã xác minh SHA-256 cho
lane preview chuyên biệt và kiểm tra một lượt trao đổi CNP với đồ thị rỗng
trước khi báo sẵn sàng. Bước này chuẩn bị bộ kiểm tra preview và pipe riêng.
Worker Run thuộc pool riêng. Dừng qua supervisor thu hồi cả hai pool.

Endpoint nội bộ `cirnet.hardware.internal.v1.SimulatorPreview` có phiên bản
riêng. `Describe` quảng bá số nguyên `1`. `Preview` chuyển JSON đồ thị
và kết quả dưới dạng trường byte protobuf. Remote báo `HARDWARE_UNAVAILABLE`
cho xem trước vật lý khi `Describe` trả `UNIMPLEMENTED`. Phiên bản công bố khác
hoặc thiếu `Preview` sau khi thương lượng thành công là lỗi giao thức.
Hardware dùng CNP1 cho frame số và CNP2 cho kế hoạch có phép chiếu DC.
Bước xác thực phản hồi kiểm tra tag, ID yêu cầu và generation được trả lại.
Kiểm chứng phát hành thử đối chiếu NCAP số và DC có cấp nguồn trên đúng mã
băm worker đã chọn.
[Định nghĩa Preview nội bộ](../../../proto/preview.proto) · [Bộ kiểm thử worker](../../../tests/simulator_parity.rs)

Start native từ chối công việc vượt hạn mức ngay để dành luồng
RPC cho Stop và reset. Cả hai vô hiệu hóa kết quả đang chờ thuộc lượt chạy cũ.
Nếu tác vụ native còn chạy, Hardware chỉ xác nhận an toàn sau khi xác minh
việc hủy và dọn dẹp. Nếu chưa thể xác minh, Hardware từ chối xác nhận và giữ
ràng buộc chủ phiên để Remote có thể cách ly lease.

Khi khôi phục phiên bản, dừng tiếp nhận yêu cầu và chờ xác nhận dọn dẹp an toàn hoặc cách ly trước khi khôi phục bộ artifact Hardware, Contracts và Simulator tương thích cùng các phiên bản được ghim ở bên tiêu thụ. Giữ cơ sở dữ liệu, bản ghi đo, quyền sử dụng trạm và trạng thái cách ly. Không phát lại Start trong quá trình khôi phục.
