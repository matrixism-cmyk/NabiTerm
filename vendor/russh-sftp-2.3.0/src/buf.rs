use bytes::Buf;

use crate::error::Error;

pub trait TryBuf: Buf {
    fn try_get_bytes(&mut self) -> Result<Vec<u8>, Error>;
    fn try_get_string(&mut self) -> Result<String, Error>;
}

impl<T: Buf> TryBuf for T {
    fn try_get_bytes(&mut self) -> Result<Vec<u8>, Error> {
        let len = self
            .try_get_u32()
            .map_err(|e| Error::UnexpectedBehavior(e.to_string()))? as usize;
        if self.remaining() < len {
            return Err(Error::BadMessage("no remaining for vec".to_owned()));
        }

        Ok(self.copy_to_bytes(len).to_vec())
    }

    fn try_get_string(&mut self) -> Result<String, Error> {
        let bytes = self.try_get_bytes()?;
        // nabiTerm 벤더 패치: UTF-8 lossy 고정 → charset 모듈(레거시 인코딩 폴백) 경유.
        Ok(crate::charset::decode(&bytes))
    }
}
