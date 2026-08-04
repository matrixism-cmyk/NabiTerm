//! nabiPad 추가 코덱(3) — 모스 부호·A1Z26 암호·천단위 구분자. 순수 함수(CyberChef 벤치마킹).

/// 한 글자의 모스 부호(영문/숫자). 그 외는 None.
fn morse_of(c: char) -> Option<&'static str> {
    Some(match c.to_ascii_uppercase() {
        'A' => ".-", 'B' => "-...", 'C' => "-.-.", 'D' => "-..", 'E' => ".", 'F' => "..-.",
        'G' => "--.", 'H' => "....", 'I' => "..", 'J' => ".---", 'K' => "-.-", 'L' => ".-..",
        'M' => "--", 'N' => "-.", 'O' => "---", 'P' => ".--.", 'Q' => "--.-", 'R' => ".-.",
        'S' => "...", 'T' => "-", 'U' => "..-", 'V' => "...-", 'W' => ".--", 'X' => "-..-",
        'Y' => "-.--", 'Z' => "--..", '0' => "-----", '1' => ".----", '2' => "..---",
        '3' => "...--", '4' => "....-", '5' => ".....", '6' => "-....", '7' => "--...",
        '8' => "---..", '9' => "----.", _ => return None,
    })
}

fn char_of_morse(code: &str) -> Option<char> {
    ('A'..='Z').chain('0'..='9').find(|&c| morse_of(c) == Some(code))
}

/// 텍스트 → 모스 부호(글자=공백, 단어=` / `).
pub(crate) fn morse_encode(t: &str) -> String {
    t.split_whitespace()
        .map(|w| w.chars().filter_map(morse_of).collect::<Vec<_>>().join(" "))
        .filter(|w| !w.is_empty())
        .collect::<Vec<_>>()
        .join(" / ")
}

/// 모스 부호 → 텍스트(`/`=단어 구분).
pub(crate) fn morse_decode(t: &str) -> String {
    t.split('/')
        .map(|word| word.split_whitespace().filter_map(char_of_morse).collect::<String>())
        .filter(|w| !w.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

/// A1Z26: 영문자 → 1~26 숫자(공백 구분, 그 외 문자 무시).
pub(crate) fn a1z26_encode(t: &str) -> String {
    t.chars()
        .filter(char::is_ascii_alphabetic)
        .map(|c| (c.to_ascii_lowercase() as u8 - b'a' + 1).to_string())
        .collect::<Vec<_>>()
        .join(" ")
}

/// A1Z26 해제: 1~26 숫자 → 영문자.
pub(crate) fn a1z26_decode(t: &str) -> String {
    t.split(|c: char| !c.is_ascii_digit())
        .filter_map(|s| s.parse::<u8>().ok())
        .filter(|&n| (1..=26).contains(&n))
        .map(|n| (b'a' + n - 1) as char)
        .collect()
}

fn group3(digits: &str) -> String {
    let n = digits.len();
    let mut o = String::with_capacity(n + n / 3);
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (n - i).is_multiple_of(3) {
            o.push(',');
        }
        o.push(c);
    }
    o
}

/// 정수 부분 숫자에 천단위 쉼표 추가(소수부는 그대로).
pub(crate) fn thousands_add(t: &str) -> String {
    let ch: Vec<char> = t.chars().collect();
    let (mut out, mut i) = (String::new(), 0);
    while i < ch.len() {
        if ch[i].is_ascii_digit() {
            let start = i;
            while i < ch.len() && ch[i].is_ascii_digit() {
                i += 1;
            }
            let run: String = ch[start..i].iter().collect();
            if start > 0 && ch[start - 1] == '.' {
                out.push_str(&run); // 소수부는 그룹화 안 함.
            } else {
                out.push_str(&group3(&run));
            }
        } else {
            out.push(ch[i]);
            i += 1;
        }
    }
    out
}

/// 숫자 사이의 천단위 쉼표 제거.
pub(crate) fn thousands_remove(t: &str) -> String {
    let ch: Vec<char> = t.chars().collect();
    ch.iter()
        .enumerate()
        .filter(|&(i, &c)| {
            !(c == ',' && i > 0 && i + 1 < ch.len() && ch[i - 1].is_ascii_digit() && ch[i + 1].is_ascii_digit())
        })
        .map(|(_, &c)| c)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn morse_roundtrip() {
        assert_eq!(morse_encode("SOS"), "... --- ...");
        assert_eq!(morse_encode("Hi there"), ".... .. / - .... . .-. .");
        assert_eq!(morse_decode("... --- ..."), "SOS");
        assert_eq!(morse_decode(".... .. / - .... . .-. ."), "HI THERE");
    }

    #[test]
    fn a1z26() {
        assert_eq!(a1z26_encode("abc XY"), "1 2 3 24 25");
        assert_eq!(a1z26_decode("1 2 3"), "abc");
    }

    #[test]
    fn thousands() {
        assert_eq!(thousands_add("price 1234567.89!"), "price 1,234,567.89!");
        assert_eq!(thousands_remove("1,234,567.89"), "1234567.89");
    }
}
