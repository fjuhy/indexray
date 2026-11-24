<p align="center"><img src="assets/128x128@2x.png" width="88" height="88" alt="IndexRay 로고"></p>
<h1 align="center">IndexRay</h1>
<p align="center">Windows에서 문서와 Outlook 메일의 본문까지 찾는다.</p>
<p align="center"><a href="https://github.com/fjuhy/indexray/releases/latest">Windows x64 다운로드</a> · <a href="https://github.com/fjuhy/indexray/releases">릴리즈 기록</a> · <a href="Guide.md">사용 안내</a></p>

## 무엇을 찾나

| 자료 | 검색과 활용 |
|---|---|
| PDF·Word·Excel·PowerPoint | 파일명, 경로, 문서 본문을 검색하고 일치한 구절을 미리 본다. |
| Outlook PST·OST | 메일 제목, 발신자, 본문과 지원 형식의 첨부파일 텍스트를 검색한다. |
| 이미지 | Windows OCR에서 읽은 텍스트를 검색한다. |
| Markdown·텍스트 | 변경 내용을 로컬 색인에 반영한다. |

## 설치하고 검색하기

1. Windows x64 설치 파일을 실행하거나 포터블 ZIP을 풀고 `IndexRay.exe`를 연다.
2. 색인 폴더 및 설정에서 폴더를 목록에 추가한다. 제외 경로와 확장자를 정한 뒤 **규칙 적용 및 새 폴더 색인 시작**을 누른다.
3. 파일명, 본문, 확장자 입력칸에서 범위를 좁힌다. `Enter`로 원본을 열고 `F1`로 검색 예시를 확인한다.

```powershell
indexray-cli search 'ext:pdf,pptx AND body:"test report"'
indexray-cli search 'ext:pst AND body:"회의 결과"'
```

## 에이전트에서 검색하기

로컬 에이전트는 `indexray-cli mcp`에 연결해 색인을 검색한다. Markdown 내보내기를 켜면 문서와 개별 메일을 지정한 폴더에 저장한다. 지원 형식의 첨부파일 텍스트도 메일 파일에 담긴다. 설치 파일과 포터블 ZIP에 `Guide.md`가 들어 있다.

색인과 검색은 PC에서 실행한다. OneDrive를 출력 폴더로 지정하면 동기화와 공유 권한은 OneDrive 설정을 따른다. 이미지로만 구성된 PDF에는 OCR을 적용하지 않는다.

## 소스에서 빌드하기

Windows x64에서 Rust MSVC 툴체인, Visual Studio C++ 빌드 도구, Node.js, Python 3.11 이상과 NSIS를 준비한다. `powershell -File build.ps1`을 실행하면 `release/`에 설치 파일, 포터블 ZIP과 SHA-256 목록을 생성한다.

## 라이선스

IndexRay는 [Dowse 엔진 포크](https://github.com/fjuhy/dowse)를 사용하는 Windows 앱이다. `Cargo.toml`과 `Cargo.lock`에 엔진 커밋을 고정한다. 앱은 [MIT](LICENSE-MIT)로 배포하며 원본 저작권 고지를 유지한다. 번들 의존성과 한국어 사전에는 각 구성 요소의 라이선스가 적용된다.
