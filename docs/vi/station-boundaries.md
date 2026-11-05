# Ranh giới trạm và an toàn

[English](../en/station-boundaries.md) · Tiếng Việt

[Mục lục tài liệu](index.md)

Hardware phụ trách RPC cục bộ 2.0, vòng đời trạm và giao tiếp FPGA nội bộ. Remote phụ trách tiếp nhận yêu cầu, phân quyền API, lưu giữ quyền sử dụng trạm và trạng thái cách ly, lưu bản ghi đo và xác nhận giải phóng an toàn. Xem [kiến trúc Lab-First](https://github.com/cirnetlab/cirnet-remote/blob/0d038c7aed15798696223558b4e58f5ffe926aa9/docs/ARCHITECTURE.md) và [quy định an toàn vật lý](https://github.com/cirnetlab/cirnet-remote/blob/0d038c7aed15798696223558b4e58f5ffe926aa9/docs/PHYSICAL_LAB_SAFETY.md) tại bản sửa đổi đã ghim.

| Trạm | Hành vi hiện tại |
| --- | --- |
| Ảo | Mỗi thí nghiệm có phiên độc lập. Nhóm tiến trình con có giới hạn chạy backend Simulator đã chọn. Backend `legacy` cung cấp bốn kênh Logic Analyzer, không có kênh Scope hoặc Generator. `ngspice` báo không khả dụng. Stop và reset vô hiệu hóa kết quả đang chờ thuộc lượt chạy cũ. |
| Vật lý | Khi thiếu trình điều khiển, trạm báo `OFFLINE` hoặc không khả dụng, số slot và kênh bằng 0, và `hardware_applied=false`. Bộ điều hợp Serial và USB-SPI mới là chỗ dành cho tích hợp sau này. |

Start trên trạm vật lý đòi hỏi kết quả quét và an toàn còn hiệu lực, kế hoạch thực thi khớp trạm và thí nghiệm, đủ bằng chứng về điểm cuối và tài nguyên, định tuyến đã áp dụng, nguồn điện có bảo vệ và bộ thu đã sẵn sàng. An toàn, tô pô và tính đúng chức năng là các quyết định riêng. Chỉ riêng việc chưa nhận diện linh kiện hoặc tô pô không khớp chưa đủ để kết luận về an toàn điện.

Dữ liệu NCAP số đi trong `OperationReply.capture_payload`. `result_json` đi kèm chứa siêu dữ liệu. Kiểm thử phần mềm, trình điều khiển thử nghiệm và mô phỏng RTL không chứng nhận bảo vệ điện, ABI phần cứng, thu nhận tín hiệu thật hoặc thời gian đáp ứng.

Khi khôi phục phiên bản, dừng tiếp nhận yêu cầu và chờ xác nhận dọn dẹp an toàn hoặc cách ly trước khi khôi phục bộ artifact Hardware, Contracts và Simulator tương thích cùng các phiên bản được ghim ở bên tiêu thụ. Giữ cơ sở dữ liệu, bản ghi đo, quyền sử dụng trạm và trạng thái cách ly. Không phát lại Start trong quá trình khôi phục.
