import { useId, useState, type ReactNode } from "react";
import { IconEye, IconEyeOff } from "./Icons";
import { passwordStrength } from "../lib/format";

export function Toggle({
  checked,
  onChange,
  label,
  hint,
  disabled,
}: {
  checked: boolean;
  onChange: (v: boolean) => void;
  label: ReactNode;
  hint?: ReactNode;
  disabled?: boolean;
}) {
  const id = useId();
  return (
    <div className={`toggle-row ${disabled ? "is-disabled" : ""}`}>
      <div className="toggle-text">
        <label htmlFor={id}>{label}</label>
        {hint && <div className="hint">{hint}</div>}
      </div>
      <button
        id={id}
        type="button"
        role="switch"
        aria-checked={checked}
        disabled={disabled}
        className={`switch ${checked ? "on" : ""}`}
        onClick={() => onChange(!checked)}
      >
        <span className="knob" />
      </button>
    </div>
  );
}

export function Segmented<T extends string>({
  value,
  options,
  onChange,
  ariaLabel,
}: {
  value: T;
  options: { value: T; label: string; hint?: string }[];
  onChange: (v: T) => void;
  ariaLabel: string;
}) {
  return (
    <div className="segmented" role="radiogroup" aria-label={ariaLabel}>
      {options.map((o) => (
        <button
          key={o.value}
          type="button"
          role="radio"
          aria-checked={value === o.value}
          title={o.hint}
          className={value === o.value ? "active" : ""}
          onClick={() => onChange(o.value)}
        >
          {o.label}
        </button>
      ))}
    </div>
  );
}

export function PasswordInput({
  value,
  onChange,
  placeholder,
  showStrength,
  autoFocus,
  invalid,
}: {
  value: string;
  onChange: (v: string) => void;
  placeholder?: string;
  showStrength?: boolean;
  autoFocus?: boolean;
  invalid?: boolean;
}) {
  const [visible, setVisible] = useState(false);
  const strength = passwordStrength(value);
  const labels = ["Weak", "Fair", "Good", "Strong"];
  return (
    <div className="password">
      <div className={`input-wrap ${invalid ? "invalid" : ""}`}>
        <input
          type={visible ? "text" : "password"}
          value={value}
          placeholder={placeholder}
          autoFocus={autoFocus}
          autoComplete="new-password"
          spellCheck={false}
          onChange={(e) => onChange(e.target.value)}
        />
        <button
          type="button"
          className="icon-btn"
          aria-label={visible ? "Hide password" : "Show password"}
          onClick={() => setVisible((v) => !v)}
        >
          {visible ? <IconEyeOff size={16} /> : <IconEye size={16} />}
        </button>
      </div>
      {showStrength && value && (
        <div className={`strength s${strength}`}>
          <div className="bars">
            {[0, 1, 2, 3].map((i) => (
              <span key={i} className={i <= strength ? "fill" : ""} />
            ))}
          </div>
          <span>{labels[strength]}</span>
        </div>
      )}
    </div>
  );
}
