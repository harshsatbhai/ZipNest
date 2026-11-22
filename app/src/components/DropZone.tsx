import type { ReactNode } from "react";

export function DropZone({
  active,
  icon,
  title,
  subtitle,
  children,
}: {
  active: boolean;
  icon: ReactNode;
  title: string;
  subtitle: string;
  children?: ReactNode;
}) {
  return (
    <div className={`dropzone ${active ? "active" : ""}`}>
      <div className="dz-icon">{icon}</div>
      <h2>{title}</h2>
      <p className="muted">{subtitle}</p>
      {children && <div className="dz-actions">{children}</div>}
    </div>
  );
}
