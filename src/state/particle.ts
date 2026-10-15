// 버전·숫자 뒤 조사 — 마지막 숫자를 읽는 소리로 고름(0 영·1 일·3 삼·6 육·7 칠·8 팔은 받침이 있음).
const BATCHIM = new Set(["0", "1", "3", "6", "7", "8"]);
const RIEUL = new Set(["1", "7", "8"]); // 일·칠·팔 — ㄹ 받침이라 '로'

function last(word: string): string {
  return word.trim().slice(-1);
}

/** 이/가 */
export function iga(word: string): string {
  return word + (BATCHIM.has(last(word)) ? "이" : "가");
}

/** 으로/로 */
export function euro(word: string): string {
  const c = last(word);
  return word + (BATCHIM.has(c) && !RIEUL.has(c) ? "으로" : "로");
}
