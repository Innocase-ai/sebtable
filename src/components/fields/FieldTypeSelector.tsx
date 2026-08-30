import type { FieldType } from "../../types/field";

const TYPES: { value: FieldType; label: string }[] = [
  { value: "text", label: "Texte" },
  { value: "long_text", label: "Texte long" },
  { value: "number", label: "Nombre" },
  { value: "checkbox", label: "Case à cocher" },
  { value: "select", label: "Sélection" },
  { value: "date", label: "Date" },
  { value: "email", label: "Email" },
  { value: "url", label: "URL" },
  { value: "phone", label: "Téléphone" },
  { value: "attachment", label: "Pièce jointe" },
  { value: "button", label: "Bouton" },
  { value: "link", label: "Lien" },
  { value: "lookup", label: "Recherche (lookup)" },
  { value: "rollup", label: "Récap (rollup)" },
  { value: "count", label: "Compte" },
  { value: "formula", label: "Formule" },
];

// Types nécessitant une cible/config : inutilisables à la création d'une table.
const SPECIAL_TYPES = new Set<FieldType>([
  "link",
  "lookup",
  "rollup",
  "count",
  "formula",
  "button",
]);

export default function FieldTypeSelector({
  value,
  onChange,
  storedOnly = false,
}: {
  value: FieldType;
  onChange: (t: FieldType) => void;
  storedOnly?: boolean;
}) {
  const types = storedOnly ? TYPES.filter((t) => !SPECIAL_TYPES.has(t.value)) : TYPES;
  return (
    <select value={value} onChange={(e) => onChange(e.target.value as FieldType)}>
      {types.map((t) => (
        <option key={t.value} value={t.value}>
          {t.label}
        </option>
      ))}
    </select>
  );
}

export function fieldTypeLabel(type: FieldType): string {
  return TYPES.find((t) => t.value === type)?.label ?? type;
}
