# Viết và duy trì tài liệu CIRNET Hardware

[English](../../en/contributing/writing-documentation.md) · [Mục lục](../index.md)

Tuân theo [chỉ dẫn của kho](../../../AGENTS.md) và
[quy ước chung của CIRNET](https://github.com/cirnetlab/cirnet-rules/blob/master/vi/policies/documentation.md).
Giữ README gốc bằng tiếng Anh và liên kết đến cả hai mục lục. Dùng cùng
đường dẫn tương đối cho trang tiếng Anh và tiếng Việt hiện hành, rồi cập
nhật chúng cùng nhau. Giữ nguyên lệnh, định danh, phiên bản, giới hạn
và trạng thái khả năng giữa hai ngôn ngữ.

Đặt hướng dẫn về vòng đời trạm, Hardware RPC cục bộ và đường truyền FPGA riêng tại đây. Remote phụ trách tiếp nhận qua API, lease bền vững, cách ly và triển khai. Contracts sở hữu schema và binding đã sinh. Simulator sở hữu phần triển khai backend và worker.

Dùng [Task Continuity](https://github.com/cirnetlab/cirnet-rules/blob/master/vi/docs/distribution.md#task-continuity) cho kế hoạch tạm thời, tiêu chí nghiệm thu và tiến độ. Giữ đặc tả thiết kế độc lập và kế hoạch dài hạn tại repo sở hữu rồi liên kết từ checkpoint. Đánh dấu hoàn tất sau khi kiểm chứng.

Sau khi kiểm chứng thay đổi, cập
nhật hai trang tương ứng và [nhật ký phát hành](../../changelogs/README.md).

Giữ hồ sơ theo ngày trong bản lưu đường cơ sở. Dẫn revision nguồn chính xác
cho kế hoạch hoặc đặc tả đã được thay thế. Lưu nhật ký thô và đầu ra bản dựng
trong phiên ngoài kho do mình sở hữu. Trước khi bàn giao, đối chiếu cặp
ngôn ngữ, kiểm tra liên kết và anchor, rồi rà soát Git diff và đầu ra
Git bỏ qua. [Hướng dẫn kiểm chứng](verification.md)
