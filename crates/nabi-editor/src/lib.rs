//! nabi-editor — nabiPad 편집기 코어(T5-1로 nabi-app에서 분리).
//!
//! 문서 모델(EditorDoc)·렌더(editortab)·HEX(edithex*)·대용량 rope(editbuf*)·변환 도구·
//! 구문 강조(editorsyntax/hl*)·인코딩 목록까지 편집기 전부가 여기 산다. 앱은 열기/저장/
//! 닫기/원격 글루(nabi-app editor{open,save,close,sftp,app}.rs)만 가진다.

pub mod editbig;
pub mod editbuf;
pub mod editbufboxsel;
pub mod editbufmatch;
pub mod editbufvcursor;
pub mod eolmix;
pub mod guides;
pub mod hexdiff;
pub mod rulers;
pub mod wrapcol;
mod editortabguide;
mod editortabws;
#[cfg(test)]
mod editbufvcursortest;
#[cfg(test)]
mod editbufmatchtest;
pub mod editbufcol;
pub mod editbufbar;
pub mod editbuffold;
pub mod editbufedit;
pub mod editbufkeys;
pub mod editbufmenu;
pub mod editbufmove;
pub mod editbufpaint;
pub mod editbufview;
pub mod editbufxform;
pub mod hexdata;
pub mod edithex;
pub mod edithexedit;
pub mod edithexfind;
pub mod edithexmenu;
pub mod edithexops;
pub mod edithexview;
pub mod editload;
pub mod textbar;
pub mod textbuf;
pub mod textbufedit;
pub mod textdata;
pub mod textkeys;
pub mod textindex;
pub mod textview;
pub mod editmenugroups;
pub mod editor;
pub mod editoralign;
pub mod editorcase;
pub mod editorcodec;
pub mod editorcodec2;
pub mod editorcodec3;
pub mod editorcodec4;
pub mod editorcode;
pub mod editorcolor;
pub mod editorcomment;
pub mod editorconvert;
pub mod editorcsv;
pub mod editorcsv2;
pub mod editorctx;
pub mod editordev;
pub mod editordev2;
pub mod editorextra;
pub mod editorextract;
pub mod editorfind;
pub mod editorfreq;
pub mod editorhash;
pub mod editorhl;
pub mod editorhlinc;
pub mod editorhlspans;
pub mod editorindent;
pub mod editorlineops;
pub mod editorlines;
pub mod editorlist;
pub mod editorloc;
pub mod editormd5;
pub mod editormenu;
pub mod editorminimap;
pub mod editornum;
pub mod editornumops;
pub mod editoroutline;
pub mod editorreplace;
pub mod editorsort;
pub mod editorstats;
pub mod editorstatus;
pub mod editorsyntax;
pub mod editortab;
pub mod editortext;
pub mod editoruuid;
pub mod editorwidth;
pub mod editorxform;
pub mod editorxml;
pub mod editsel;
pub mod encodings;
pub mod encdetect;
pub mod humanfmt;
pub mod lspclient;
pub mod lspcomp;
pub mod lspframe;
pub mod lspread;
pub mod ropehl;
pub mod ropets;
pub mod textpos;
pub mod uiutil;
#[cfg(test)]
mod editornum_tests;

