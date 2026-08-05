//! SFTP 관련 i18n 항목(파일 작업·권한·전송 대화상자) — 카탈로그 크기 규율로 분리.

pub const CATALOG_SFTP: &[(&str, &str, &str, &str)] = &[
    ("sftp.batchtokens", "{n}=number, {nn}/{nnn}=zero-padded, {name}=base, {ext}=extension", "{n}=순번, {nn}/{nnn}=0채움, {name}=이름, {ext}=확장자", "{n}=連番, {nn}/{nnn}=ゼロ埋め, {name}=名前, {ext}=拡張子"),
    ("sessions.corrupt", "Session file was damaged — original kept at", "세션 파일이 손상되어 원본을 보관했습니다", "セッションファイルが破損 — 原本を保存しました"),
    ("find.filtersave", "Turn off the line filter before saving — only matching lines are shown", "줄 필터를 끄고 저장하세요 — 지금은 일치하는 줄만 표시됩니다", "行フィルタを解除してから保存してください — 現在は一致行のみ表示中"),
    ("sftp.syncpreview", "files to sync:", "동기화할 파일:", "同期するファイル:"),
    ("sftp.syncapply", "Apply", "적용", "適用"),
    ("sftp.dirsize", "Folder size", "폴더 크기", "フォルダサイズ"),
    ("sftp.openterm", "Open terminal here", "여기서 터미널 열기", "ここでターミナルを開く"),
    ("sftp.reconnect", "Reconnect", "재연결", "再接続"),
    ("sftp.chmodrec", "Apply to all subfolders", "하위 포함 적용", "サブフォルダにも適用"),
    ("sftp.deletedir", "Delete folder and all its contents", "폴더와 하위 전체 삭제", "フォルダと中身を全て削除"),
    ("sftp.deletemulti", "Delete all selected items", "선택 항목 전체 삭제", "選択項目を全て削除"),
    ("sftp.calcsize", "Calculating size…", "크기 계산 중…", "サイズ計算中…"),
    ("sftp.overwrite.title", "File already exists", "파일이 이미 있음", "ファイルが既に存在"),
    ("sftp.overwrite.body", "{n} file(s) already exist in the destination.\nYes = overwrite · No = skip existing · Cancel = abort", "대상 폴더에 이미 {n}개 파일이 있습니다.\n예 = 덮어쓰기 · 아니오 = 기존 건너뜀 · 취소 = 중단", "宛先に既に{n}個のファイルがあります。\nはい=上書き · いいえ=既存をスキップ · キャンセル=中止"),
    // v0.1.409 — 툴바 그룹화(설명 없는 아이콘 줄이기).
    ("sftp.home", "Home folder", "홈 폴더", "ホームフォルダ"),
    ("sftp.close", "Close this connection", "이 연결 닫기", "この接続を閉じる"),
    ("sftp.bookmarks", "Bookmarks", "즐겨찾기", "ブックマーク"),
    ("sftp.syncgroup", "Compare / sync", "비교·동기화", "比較・同期"),
    ("sftp.tools", "Tools", "도구", "ツール"),
];
