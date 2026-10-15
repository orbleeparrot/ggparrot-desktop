// 창 위로 파일을 끌고 오는 동안만 보이는 안내. 실제 가져오기는 Rust가 하고 runner:notice(info)로 알림.
import { FileDown } from "lucide-react";
import { useEffect, useRef, useState } from "react";

function hasFiles(e: DragEvent): boolean {
  return !!e.dataTransfer && Array.from(e.dataTransfer.types ?? []).includes("Files");
}

export function DropOverlay() {
  const [shown, setShown] = useState(false);
  const depth = useRef(0);

  useEffect(() => {
    function enter(e: DragEvent) {
      if (!hasFiles(e)) return;
      depth.current += 1;
      setShown(true);
    }
    function over(e: DragEvent) {
      if (!hasFiles(e)) return;
      e.preventDefault();
      setShown(true);
    }
    function leave(e: DragEvent) {
      if (!hasFiles(e)) return;
      depth.current = Math.max(0, depth.current - 1);
      if (depth.current === 0) setShown(false);
    }
    function drop(e: DragEvent) {
      if (hasFiles(e)) e.preventDefault();
      depth.current = 0;
      setShown(false);
    }
    window.addEventListener("dragenter", enter);
    window.addEventListener("dragover", over);
    window.addEventListener("dragleave", leave);
    window.addEventListener("drop", drop);
    return () => {
      window.removeEventListener("dragenter", enter);
      window.removeEventListener("dragover", over);
      window.removeEventListener("dragleave", leave);
      window.removeEventListener("drop", drop);
    };
  }, []);

  if (!shown) return null;
  return (
    <div className="drop-overlay" aria-hidden="true">
      <div className="drop-target">
        <FileDown size={28} />
        <p>여기에 놓으면 매크로 폴더로 가져와요</p>
        <span>.ggm.json · .json</span>
      </div>
    </div>
  );
}
