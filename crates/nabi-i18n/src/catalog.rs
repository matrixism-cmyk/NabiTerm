//! 번역 카탈로그(키, en, ko, ja).

use crate::Lang;

const CATALOG: &[(&str, &str, &str, &str)] = &[
    ("menu.file", "File", "파일", "ファイル"),
    ("menu.view", "View", "보기", "表示"),
    ("menu.newlocal", "New Local Terminal", "새 로컬 터미널", "新しいローカル端末"),
    (
        "menu.quickconnect",
        "Quick Connect (SSH)...",
        "빠른 연결 (SSH)...",
        "クイック接続 (SSH)...",
    ),
    ("menu.exit", "Exit", "종료", "終了"),
    ("menu.configdir", "Open Config Folder", "설정 폴더 열기", "設定フォルダを開く"),
    ("menu.saveoutput", "Save terminal output", "터미널 출력 저장", "ターミナル出力を保存"),
    ("menu.saveworkspace", "Save Workspace", "워크스페이스 저장", "ワークスペース保存"),
    (
        "menu.restoreworkspace",
        "Restore Workspace",
        "워크스페이스 복원",
        "ワークスペース復元",
    ),
    ("menu.zoomin", "Zoom In", "확대", "拡大"),
    ("menu.zoomout", "Zoom Out", "축소", "縮小"),
    ("menu.language", "Language", "언어", "言語"),
    ("menu.sessions", "Sessions", "세션 관리", "セッション管理"),
    ("menu.browser", "File Browser", "파일 브라우저", "ファイルブラウザ"),
    ("browser.cdhere", "Open in terminal", "터미널에서 열기", "端末で開く"),
    ("status.opencwd", "Open in file browser", "파일 브라우저에서 열기", "ファイルブラウザで開く"),
    ("browser.filter", "Filter…", "필터…", "フィルタ…"),
    ("menu.tearoff", "Tear Tab into New Window", "탭을 새 창으로 분리", "タブを新しいウィンドウへ"),
    ("menu.arrange", "Arrange Windows", "창 정렬", "ウィンドウ整列"),
    ("arrange.tile", "Tile", "바둑판식 정렬", "並べて表示"),
    ("arrange.cascade", "Cascade", "계단식 정렬", "重ねて表示"),
    (
        "menu.broadcast",
        "Broadcast Input (MultiExec)",
        "입력 브로드캐스트 (MultiExec)",
        "入力ブロードキャスト (MultiExec)",
    ),
    ("status.broadcast", "BROADCAST", "브로드캐스트", "ブロードキャスト"),
    ("tab.rename", "Rename tab:", "탭 이름 변경:", "タブ名変更:"),
    ("tab.reset", "Reset name", "이름 초기화", "名前リセット"),
    ("tab.color", "Color:", "색:", "色:"),
    ("tab.duplicate", "Duplicate tab", "탭 복제", "タブを複製"),
    ("tab.reopen", "Reopen closed tab", "닫은 탭 다시 열기", "閉じたタブを再度開く"),
    ("tab.closeothers", "Close other tabs", "다른 탭 모두 닫기", "他のタブを閉じる"),
    ("tab.close", "Close tab", "탭 닫기", "タブを閉じる"),
    ("tab.selectall", "Select all (copy)", "전체 선택(복사)", "全選択(コピー)"),
    ("menu.zoompane", "Maximize pane (zoom)", "페인 최대화(줌)", "ペイン最大化(ズーム)"),
    ("term.reset", "Reset terminal", "터미널 리셋", "端末リセット"),
    (
        "tab.bcastgroup",
        "In broadcast group",
        "브로드캐스트 그룹에 포함",
        "ブロードキャスト対象",
    ),
    ("find.title", "Find", "찾기", "検索"),
    ("find.hint", "Search…", "검색…", "検索…"),
    ("palette.title", "Command Palette", "명령 팔레트", "コマンドパレット"),
    (
        "palette.hint",
        "Type a command…  (Ctrl+Shift+P, Esc to close)",
        "명령 입력…  (Ctrl+Shift+P, Esc로 닫기)",
        "コマンド入力…  (Ctrl+Shift+P, Escで閉じる)",
    ),
    ("sessions.empty", "(none saved)", "(저장된 항목 없음)", "(保存なし)"),
    ("sessions.delete", "Delete session", "세션 삭제", "セッション削除"),
    ("sessions.edit", "Edit session", "세션 편집", "セッション編集"),
    ("sessions.duplicate", "Duplicate", "복제", "複製"),
    ("menu.newssh", "+ New SSH Connection", "+ 새 SSH 연결", "+ 新規SSH接続"),
    (
        "menu.exportsessions",
        "Export Sessions…",
        "세션 내보내기…",
        "セッションを書き出し…",
    ),
    ("qc.save", "Save", "저장", "保存"),
    ("menu.settings", "Settings", "설정", "設定"),
    ("settings.title", "Preferences", "환경설정", "環境設定"),
    ("settings.fontsize", "Font size", "글꼴 크기", "フォントサイズ"),
    ("settings.language", "Language", "언어", "言語"),
    ("settings.shell", "Default shell", "기본 셸", "既定シェル"),
    ("settings.scrollback", "Scrollback", "스크롤백", "スクロールバック"),
    ("settings.theme", "Color theme", "색 테마", "カラーテーマ"),
    ("settings.cursorblink", "Cursor blink", "커서 깜빡임", "カーソル点滅"),
    ("settings.uiscale", "UI scale", "UI 배율", "UI 倍率"),
    ("settings.encoding", "Encoding", "인코딩", "エンコーディング"),
    (
        "notify.bgfail",
        "Background command failed",
        "백그라운드 명령 실패",
        "バックグラウンドコマンド失敗",
    ),
    (
        "settings.fontfamily",
        "Font file (restart)",
        "글꼴 파일(재시작)",
        "フォントファイル(再起動)",
    ),
    ("settings.cursorshape", "Cursor shape", "커서 모양", "カーソル形状"),
    ("settings.cursorcolor", "Cursor color", "커서 색", "カーソル色"),
    ("settings.selectioncolor", "Selection color", "선택 색", "選択色"),
    ("settings.matchcolor", "Match color", "검색 색", "検索色"),
    ("settings.fgcolor", "Text color", "글자 색", "文字色"),
    ("settings.bgcolor", "Background color", "배경 색", "背景色"),
    ("settings.visualbell", "Visual bell", "비주얼 벨", "ビジュアルベル"),
    ("settings.statusbar", "Status bar", "상태바", "ステータスバー"),
    ("settings.warnpaste", "Confirm multiline paste", "여러 줄 붙여넣기 확인", "複数行貼り付け確認"),
    ("settings.searchlimit", "Search depth (lines)", "검색 깊이(줄)", "検索深度(行)"),
    ("settings.sec.appearance", "Appearance", "모양", "外観"),
    ("settings.sec.font", "Font & Theme", "글꼴·테마", "フォント・テーマ"),
    ("settings.sec.cursor", "Cursor", "커서", "カーソル"),
    ("settings.sec.colors", "Colors", "색상", "色"),
    ("settings.sec.terminal", "Terminal", "터미널", "ターミナル"),
    ("settings.sec.behavior", "Behavior", "동작", "動作"),
    ("central.newtab", "+ New Shell", "+ 새 셸", "+ 新規シェル"),
    ("menu.importsessions", "Import Sessions", "세션 가져오기", "セッション取り込み"),
    ("menu.importsshconfig", "Import ~/.ssh/config", "~/.ssh/config 가져오기", "~/.ssh/config取り込み"),
    ("menu.edit", "Edit", "편집", "編集"),
    ("menu.fullscreen", "Fullscreen (F11)", "전체화면 (F11)", "全画面 (F11)"),
    ("menu.splitright", "Split Right", "오른쪽 분할", "右に分割"),
    ("menu.splitdown", "Split Down", "아래로 분할", "下に分割"),
    ("menu.copy", "Copy", "복사", "コピー"),
    ("menu.paste", "Paste", "붙여넣기", "貼り付け"),
    ("menu.selectall", "Select All", "모두 선택", "すべて選択"),
    ("menu.find", "Find", "찾기", "検索"),
    ("menu.snippets", "Snippets", "스니펫", "スニペット"),
    ("menu.addsnippet", "+ Add selection as snippet", "+ 선택을 스니펫으로 추가", "+ 選択をスニペット追加"),
    ("snippets.empty", "No snippets yet — add from the menu or Settings.", "스니펫 없음 — 메뉴나 설정에서 추가.", "スニペットなし — メニューや設定から追加。"),
    ("status.cycleenc", "Click to change encoding", "클릭하면 인코딩 변경", "クリックでエンコーディング変更"),
    ("find.smartcase", "Smart case: uppercase = case-sensitive", "스마트케이스: 대문자=대소문자 구분", "スマートケース: 大文字=区別"),
    ("browser.copypath", "Click to copy path", "클릭하면 경로 복사", "クリックでパスをコピー"),
    ("paste.title", "Paste multiple lines?", "여러 줄을 붙여넣을까요?", "複数行を貼り付けますか?"),
    ("paste.body", "Clipboard has multiple lines", "클립보드에 여러 줄이 있습니다", "クリップボードに複数行があります"),
    ("paste.do", "Paste", "붙여넣기", "貼り付け"),
    ("settings.blinkms", "Blink rate (ms)", "깜빡임 속도(ms)", "点滅速度(ms)"),
    (
        "settings.quakehotkey",
        "Quake hotkey (restart)",
        "Quake 핫키(재시작)",
        "Quakeホットキー(再起動)",
    ),
    (
        "settings.confirmclose",
        "Confirm on close",
        "닫기 확인",
        "閉じる確認",
    ),
    (
        "settings.copyonselect",
        "Copy on select",
        "선택 시 복사",
        "選択でコピー",
    ),
    (
        "settings.restorews",
        "Restore tabs on start",
        "시작 시 탭 복원",
        "起動時にタブ復元",
    ),
    (
        "settings.restorecmd",
        "Re-run last running command on restore",
        "복원 시 마지막 실행 중이던 명령 재실행",
        "復元時に最後の実行中コマンドを再実行",
    ),
    ("settings.save", "Save", "저장", "保存"),
    ("settings.reset", "Reset to defaults", "기본값 복원", "既定に戻す"),
    ("menu.ontop", "Always on Top", "항상 위에", "最前面に表示"),
    ("menu.vault", "Vault", "볼트", "Vault"),
    ("menu.help", "Help", "도움말", "ヘルプ"),
    ("help.about", "About", "정보", "情報"),
    (
        "help.desc",
        "Native Windows terminal · SSH client (Rust/egui)",
        "네이티브 Windows 터미널 · SSH 클라이언트 (Rust/egui)",
        "ネイティブ Windows 端末 · SSH クライアント (Rust/egui)",
    ),
    ("help.shortcuts", "Keyboard shortcuts:", "키보드 단축키:", "キーボードショートカット:"),
    ("vault.title", "Unlock Vault", "볼트 잠금 해제", "Vault のロック解除"),
    ("vault.prompt", "Master password:", "마스터 비밀번호:", "マスターパスワード:"),
    ("vault.unlock", "Unlock", "잠금 해제", "ロック解除"),
    ("vault.create", "Create Vault", "볼트 만들기", "Vault 作成"),
    ("vault.unlocked", "Vault unlocked. Stored entries:", "볼트 잠금 해제됨. 저장 항목:", "解除済み。保存:"),
    ("vault.lock", "Lock", "잠금", "ロック"),
    (
        "vault.firstuse",
        "First use: the password you set here becomes the master password.",
        "처음 사용: 여기서 정한 비밀번호가 마스터 비밀번호가 됩니다.",
        "初回: ここで設定したパスワードがマスターになります。",
    ),
    ("vault.reset", "Reset (delete vault)", "초기화(볼트 삭제)", "リセット(削除)"),
    ("vault.remember", "Remember password (OS credential)", "비밀번호 기억(OS 자격증명)", "パスワードを記憶(OS資格情報)"),
    ("vault.remember.warn", "Stores the master password in Windows Credential Manager and auto-unlocks on start. Convenient but lower security — anyone using this Windows account can open the vault.", "마스터 비밀번호를 Windows 자격증명 관리자에 저장하고 시작 시 자동으로 잠금을 해제합니다. 편리하지만 보안은 낮아집니다 — 이 Windows 계정을 쓰는 누구나 볼트를 열 수 있습니다.", "マスターパスワードをWindows資格情報マネージャーに保存し、起動時に自動でロック解除します。便利ですが安全性は下がります — このWindowsアカウントを使う誰でも開けます。"),
    (
        "vault.reset_done",
        "Vault deleted. Set a new master password.",
        "볼트가 삭제되었습니다. 새 마스터 비밀번호를 설정하세요.",
        "削除しました。新しいパスワードを設定してください。",
    ),
    (
        "vault.wrong",
        "Wrong password or corrupt vault",
        "비밀번호가 틀렸거나 볼트가 손상됨",
        "パスワードが違うか破損",
    ),
    ("status.nosession", "No session", "세션 없음", "セッションなし"),
    ("status.sessions", "Sessions", "세션", "セッション"),
];

/// 키를 현재 언어 문자열로 변환한다(미상 키는 "?"). CATALOG + 분할된 CATALOG2를 함께 본다.
pub fn tr(lang: Lang, key: &str) -> &'static str {
    for (k, en, ko, ja) in CATALOG
        .iter()
        .chain(crate::catalog2::CATALOG2)
        .chain(crate::catalog3::CATALOG3)
        .chain(crate::catalog_editor::CATALOG_EDITOR)
        .chain(crate::catalog_editor2::CATALOG_EDITOR2)
        .chain(crate::catalog_sftp::CATALOG_SFTP)
    {
        if *k == key {
            return match lang {
                Lang::En => en,
                Lang::Ko => ko,
                Lang::Ja => ja,
            };
        }
    }
    "?"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn translates_known_keys_and_falls_back() {
        assert_eq!(tr(Lang::Ko, "menu.file"), "파일");
        assert_eq!(tr(Lang::Ja, "menu.exit"), "終了");
        assert_eq!(tr(Lang::En, "menu.view"), "View");
        assert_eq!(tr(Lang::Ko, "nonexistent.key"), "?");
    }

    #[test]
    fn catalog_integrity() {
        use std::collections::HashSet;
        let mut seen = HashSet::new();
        for (k, en, ko, ja) in CATALOG.iter().chain(crate::catalog2::CATALOG2).chain(crate::catalog3::CATALOG3).chain(crate::catalog_editor::CATALOG_EDITOR).chain(crate::catalog_editor2::CATALOG_EDITOR2)
        .chain(crate::catalog_sftp::CATALOG_SFTP) {
            assert!(seen.insert(*k), "중복 키: {k}");
            assert!(
                !en.is_empty() && !ko.is_empty() && !ja.is_empty(),
                "빈 번역: {k}"
            );
        }
    }
}
