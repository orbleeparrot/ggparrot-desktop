import type { ReactNode } from "react";
import type { SettingsSection } from "../../state/runs";

/** 설정 묶음 하나 — 제목·한 줄 설명·내용. id로 왼쪽 목차와 [설정에서 입력]이 찾아옴. */
export function Section({
  id,
  title,
  lead,
  children,
}: {
  id: SettingsSection;
  title: string;
  lead?: string;
  children: ReactNode;
}) {
  return (
    <section id={`settings-${id}`} className="settings-section" aria-labelledby={`settings-${id}-title`} tabIndex={-1}>
      <h3 id={`settings-${id}-title`} className="settings-section-title">
        {title}
      </h3>
      {lead && <p className="settings-lead">{lead}</p>}
      {children}
    </section>
  );
}
