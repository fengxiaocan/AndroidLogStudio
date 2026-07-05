interface SearchBarProps {
  value: string;
  onChange: (value: string) => void;
}

export function SearchBar({ value, onChange }: SearchBarProps) {
  return (
    <label className="field search-bar">
      <span className="field__label">Search</span>
      <input
        className="field__input"
        value={value}
        onChange={(event) => onChange(event.currentTarget.value)}
        placeholder="Ctrl+F"
      />
    </label>
  );
}
