interface QueryBarProps {
  value: string;
  onChange: (value: string) => void;
}

export function QueryBar({ value, onChange }: QueryBarProps) {
  return (
    <label className="field query-bar">
      <span className="field__label">Query Filter</span>
      <input
        className="field__input"
        value={value}
        onChange={(event) => onChange(event.currentTarget.value)}
        placeholder="package:launcher level:error tag:ActivityManager"
      />
    </label>
  );
}
