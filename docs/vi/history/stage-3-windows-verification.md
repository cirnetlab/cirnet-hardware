# Kiểm chứng ứng viên Hardware Rust

[English](../../en/history/stage-3-windows-verification.md) · Tiếng Việt

[Mục lục tài liệu](../index.md) · [Dựng và kiểm thử ứng viên](../contributing/verification.md#ứng-viên-hardware-rust)

**Gate 3 về chức năng, tương thích và an toàn trên Windows x64 đạt với
r8-R2.** Cả sáu ngưỡng Stop của Windows đều đạt. Phép chẩn đoán hiệu năng
đối sánh riêng vượt ba giới hạn tương đối. Hiệu năng Linux/Xeon, HIL vật lý
và nghiệm thu sản xuất hoặc phát hành vẫn ở trạng thái `NOT RUN`.
[Manifest phát hành CIRNET 0.1.4](https://github.com/cirnetlab/cirnet-remote/blob/38aea6120920f48cea817827ac2277cce4bef25e/integration/release-set.json) chọn cấu hình Python để khôi phục.
[Chính sách và kết quả Gate 3 của Remote](https://github.com/cirnetlab/cirnet-remote/blob/13a35145b6d15644c7be10d95a5410bf69e11867/docs/vi/architecture/native-migration.md#chính-sách-nghiệm-thu-gate-3-theo-môi-trường)

Hồ sơ dưới đây giữ nguyên thứ tự các mốc đo. Trạng thái `NOT RUN` ở mốc trước
áp dụng cho thời điểm đó. Kết quả r8-R2 cuối cùng nằm trong phần nghiệm thu
chức năng và an toàn trên Windows.

## Hồ sơ đo

### Bằng chứng thời gian Stop tùy chọn trên Windows

Trong lane kiểm chứng giai đoạn 3, lệnh `StopExecution` có thể yêu cầu thời
điểm vào handler Hardware bằng cách gửi metadata gRPC
`x-cirnet-stop-ingress` với giá trị `qpc-ns-v1`. Với ID thí nghiệm gồm 32 ký
tự thập lục phân viết thường, dịch vụ trả metadata ban đầu cùng khóa với giá
trị `qpc-ns-v1:<experiment_id>:<nanoseconds>`. Dịch vụ lấy dấu thời gian
từ QueryPerformanceCounter của Windows khi vào handler và gắn dấu sau khi
xử lý Stop. [Windows QPC](https://learn.microsoft.com/en-us/windows/win32/sysinfo/acquiring-high-resolution-time-stamps)

Harness đối chiếu dấu với ID thí nghiệm và kiểm tra thứ tự so với mốc cả bốn
worker còn sống, worker mục tiêu thoát, ACK an toàn và lease được giải phóng
bền vững. Các khoảng thời gian quan trọng phải lớn hơn một tick QPC. Kiểm
chứng trên Windows x64 đạt 12 ca vòng đời và giao thức Hardware, gồm kiểm tra
metadata trên cả hai tên RPC, và 15 ca kiểm tra tập trung của harness.

Lượt chẩn đoán r4 không tạm dừng worker đạt 3 trong 35 lượt thử với Hardware
Python B và 8 trong 8 lượt với Hardware Rust. Lane Python thiếu mẫu so với
mục tiêu tám lượt hợp lệ. Lượt đo lại Stop chính thức cho ứng viên này có
trạng thái `NOT RUN`. Gate yêu cầu ba cửa sổ với 100 lượt hợp lệ mỗi bên và
các ngưỡng p95/p99 hiện tại.

### Chẩn đoán đồng bộ Start tùy chọn trên Windows

Trong lane kiểm chứng riêng của giai đoạn 3, đặt
`CIRNET_HARDWARE_START_RENDEZVOUS=1` để bật cơ chế chờ nhóm tại điểm vào
handler `StartExecution`. Hai tên RPC dùng chung nhóm theo từng đợt.
Ticket hợp lệ trong metadata `x-cirnet-start-wave` chỉ định một đợt và
đúng bốn ID thí nghiệm ảo khác nhau. Mỗi handler chờ trước khi thao tác
với station. Yêu cầu trùng, thành viên xung đột, việc hủy được phát hiện
trước khi mở nhóm hoặc thiếu thành viên khi hết hạn hai giây sẽ hủy cả
nhóm. Ticket đã đóng giữ nguyên trạng thái trong suốt vòng đời của dịch
vụ kiểm chứng. Lệnh không mang ticket đi theo luồng Start thông thường.

Wrapper kiểm thử của Python B đã ghim và Hardware Rust đều trả bốn dấu
thời gian vào handler và một mốc mở nhóm chung bằng QPC Windows trong
metadata phản hồi gRPC. Hai bên dùng cùng cơ chế đồng bộ khi Remote gửi
yêu cầu. Các worker C++ chạy liên tục với workload đã ghim. Harness Stop
vẫn kiểm tra cả bốn worker đã đọc đủ request và chưa ghi phản hồi cho đến
khi worker mục tiêu thoát, cùng với mốc vào handler Stop khớp thí nghiệm,
ACK an toàn, giải phóng lease bền vững và kiểm tra capture.

Binary Rust Windows x64 có SHA-256
`22d3d0a1c57aad0d2d01d512f3a890149566bc799228559b6943843cba1e85e6`
đạt 15 ca RPC Hardware tập trung. Wrapper Python B và harness đạt 22 ca.
Với các wheel Remote r4, lượt chẩn đoán đạt 8 trong 10 lần thử trên Python
B (80,0%) và 8 trong 8 lần trên Rust. Lượt chẩn đoán trước khi đồng bộ
tại handler đạt 3 trong 35 lần trên Python B (8,6%) và 8 trong 8 lần trên
Rust. Hai lượt đo cặp mất lần lượt 199,80 và 180,96 giây, với số lượt hợp
lệ và lượt làm nóng khác nhau.

Cả sáu so sánh p95/p99 trong lượt chẩn đoán đều đạt ngưỡng hiện có với
tám lượt hợp lệ mỗi bên. Biên bản
`native-hardware-stop/c27bcece564342159e0726c69ff8d16d/stop-summary.json`
có SHA-256
`001033e2476c81f65aae46fdb754f38203d8e91a717c8e299a514499b6aac712`.
Lượt chẩn đoán tám mẫu này diễn ra trước lượt nghiệm thu r8–R2 bên dưới.

Chọn biến thể nghiệm thu đã được phê duyệt trong harness Remote bằng
`CIRNET_NATIVE_STAGE3_STOP_METHOD=natural-loaded-stop-hardware-rendezvous-v1`.
Phương pháp không đồng bộ tại handler là mặc định. Mỗi cửa sổ nghiệm thu
cho phép tối đa 403 lần thử để thu đủ 100 lượt hợp lệ mỗi bên. Binary cuối
đạt toàn bộ 25 ca RPC và vòng đời Hardware với bản đối chứng đã ghim. Ba
ca Remote r7 cũng đạt, gồm giải phóng lease bền vững khi hết hạn hoặc mất
kết nối và cách ly khi chưa nhận được ACK an toàn.

### Nghiệm thu chức năng và an toàn trên Windows

Phạm vi nghiệm thu trên Windows i5 là `windows-functional-safety-v1`.
Chọn bằng `CIRNET_NATIVE_STAGE3_STOP_ACCEPTANCE_SCOPE` cùng phương pháp
đồng bộ Start tại Hardware đã duyệt. Phạm vi này giữ ba cửa sổ đo cặp,
100 mẫu hợp lệ mỗi bên trong mỗi cửa sổ và tối đa 403 lần thử cho mỗi
cửa sổ. Mỗi mẫu phải chứng minh cả bốn worker đang hoạt động khi vào
handler Stop, xác nhận đã thu hồi tiến trình mục tiêu trước ACK an toàn,
đồng thời kiểm tra cách ly kết quả capture và trạng thái lease bền vững.

| Ngưỡng vận hành Stop đã chốt | p95 ms | p99 ms |
| --- | ---: | ---: |
| Stop HTTP | 197,554060 | 581,156400 |
| ACK gRPC ban đầu | 139,204780 | 526,711790 |
| Giải phóng lease bền vững | 2.534,556860 | 2.598,374680 |

Cả sáu ngưỡng đã chốt đều bắt buộc trên Windows. So sánh cặp với giới
hạn 110% của Python B được ghi riêng để chẩn đoán hiệu năng, cùng các
số đo CPU, bộ nhớ và thông lượng trên Windows. Hạn dọn dẹp của Hardware
và hạn thu hồi tiến trình vẫn bắt buộc. Khi thiếu ACK dọn dẹp hoặc chưa
xác minh được ACK, Remote giữ cách ly thay vì giải phóng lease. Nghiệm thu
hiệu năng trên Linux/Xeon có trạng thái `NOT RUN`. Cần chốt ngưỡng trước
khi nghiệm thu.

#### Kết quả Stop khi có tải của r8–R2

**PASS** trên Windows x64 theo phạm vi `windows-functional-safety-v1` và
phương pháp `natural-loaded-stop-hardware-rendezvous-v1`. Cả sáu cửa sổ
đều đủ 100 mẫu Stop hợp lệ trong giới hạn 403 lần thử. Python B cần lần
lượt 175, 153 và 174 lần thử. Rust C cần 100 lần trong mỗi cửa sổ.
Pytest chạy trong 2.953,75 giây. Số đo là trung vị của các phân vị từ ba cửa sổ.

| Thời gian Stop tính bằng ms | Python B p95/p99 | Rust C p95/p99 | Ngưỡng đã chốt |
| --- | ---: | ---: | --- |
| Stop HTTP | 78,6032 / 100,8966 | 89,1368 / 100,4389 | PASS |
| ACK gRPC ban đầu | 56,3787 / 69,3376 | 62,2677 / 78,5544 | PASS |
| Giải phóng lease bền vững | 145,9619 / 168,5140 | 144,0377 / 163,8268 | PASS |

Chẩn đoán hiệu năng cặp vượt giới hạn 110% của Python B ở HTTP p95
(89,1368 so với 86,46352 ms), ACK gRPC ban đầu p95 (62,2677 so với
62,01657 ms) và ACK gRPC ban đầu p99 (78,5544 so với 76,27136 ms).
Ba phép so sánh cặp còn lại đạt. Kết quả Stop trên Windows dựa trên
sáu ngưỡng đã chốt.

Đối chiếu chỉ đọc xác minh cả 600 mẫu. Mỗi mẫu có bốn thí nghiệm khác nhau
dùng chung mốc QPC và bốn worker chạy liên tục. Các ảnh chụp bộ đếm xác
nhận bốn yêu cầu đủ 2.806.507 byte và chưa ghi phản hồi cho đến khi tiến
trình mục tiêu thoát. Từng tiến trình mục tiêu thoát trước ACK an toàn và
trước khi lease được giải phóng bền vững, đồng thời không có capture.
Đối chiếu đã tính lại mã băm của cả 1.800 tệp NCAP từ các worker còn lại,
xác nhận không còn lease và dịch vụ đã dừng ở cả sáu lane.

Manifest đã chọn có SHA-256
`ebc49c57c052e0a78c98074af240c78010f787872a4ad32ac23617ada1fc15bd`.
Binary Hardware Rust có SHA-256
`22d3d0a1c57aad0d2d01d512f3a890149566bc799228559b6943843cba1e85e6`
và wheel Remote r8 có SHA-256
`0e3280dc41d81e1b00b4ee7a5e35a3646b06b32276181000ee874b43e113bd01`.
Biên bản `native-hardware-stop/97612df8bfb942bfbb131dbaf78c1b10/stop-summary.json`
có SHA-256
`41b41bb221ec29118e324637119cf2fdfd5a267760b2b7a1da8d9da3310593b0`.

Hiệu năng Linux/Xeon và HIL vật lý có trạng thái `NOT RUN`. Cấu hình
đã chọn công bố thực thi ảo và báo thực thi vật lý chưa khả dụng.
