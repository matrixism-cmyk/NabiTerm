# Mesa 3D (llvmpipe) — 소프트웨어 OpenGL 폴백

GPU가 없는 VM·헤드리스·드라이버가 깨진 환경에서 nabiTerm(wgpu GL 백엔드)을 실행하기 위한
**소프트웨어 OpenGL** 구현(Mesa llvmpipe). exe 옆에 두면 Windows DLL 검색 1순위라 wgpu의
GL 백엔드가 이 Mesa를 사용한다. `NABI_RENDERER=software` 로 강제할 수도 있다.

## 구성 파일(두 개 모두 필요)
- `opengl32.dll` — Gallium WGL 프런트엔드(앱 로컬 GL 오버라이드)
- `libgallium_wgl.dll` — llvmpipe(LLVM JIT 소프트웨어 래스터라이저) 포함 드라이버

## 출처·버전(재현 가능)
- 프로젝트: Mesa 3D (https://www.mesa3d.org), 라이선스 **MIT**
- Windows 빌드: pal1000/mesa-dist-win `26.1.3` 의 `mesa3d-26.1.3-release-msvc.7z` 중 `x64/`
- 다운로드: https://github.com/pal1000/mesa-dist-win/releases/tag/26.1.3

## SHA256
```
opengl32.dll        12499866437A161D2B250D5105188AE00732DD74B4BEBBCDF972E6145AF00F9E
libgallium_wgl.dll  1895F8C19EDE5EFD0497F9DFAB463B19BF4377E3AF7C06C2D4D073E4680C5F69
```

## 갱신 방법
위 릴리스에서 새 `release-msvc.7z`를 받아 `x64/opengl32.dll`·`x64/libgallium_wgl.dll`을
이 폴더에 덮어쓰고, 위 SHA256과 helppages.rs 표기를 갱신한다.
