import { useState } from "react";
import { UIInput } from "@/components/ui/UIInput";
import { UIButton } from "@/components/ui/UIButton";

interface EditProjectFormProps {
  initialName: string;
  initialPath: string | null;
  /**
   * Called with only the fields that actually changed. `newPath` set to an
   * empty string clears the project's path; `undefined` leaves it untouched.
   * Not called at all when nothing changed (the parent just closes the form).
   */
  onSubmit: (newName?: string, newPath?: string) => void;
  onCancel: () => void;
}

export function EditProjectForm({
  initialName,
  initialPath,
  onSubmit,
  onCancel,
}: EditProjectFormProps) {
  const [name, setName] = useState(initialName);
  const [path, setPath] = useState(initialPath ?? "");

  const trimmedName = name.trim();
  const trimmedPath = path.trim();
  const nameChanged = trimmedName !== initialName;
  const pathChanged = trimmedPath !== (initialPath ?? "");
  const canSubmit = trimmedName.length > 0 && (nameChanged || pathChanged);

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (!canSubmit) {
      onCancel();
      return;
    }
    onSubmit(
      nameChanged ? trimmedName : undefined,
      pathChanged ? trimmedPath : undefined,
    );
  };

  return (
    <form
      onSubmit={handleSubmit}
      className="flex flex-col gap-[var(--spacing-2)] p-[var(--spacing-3)]"
    >
      <UIInput
        label="Project name"
        placeholder="e.g. my-project"
        value={name}
        onChange={(e) => setName(e.target.value)}
        autoFocus
      />
      <UIInput
        label="Path (optional)"
        placeholder="/Users/.../project"
        value={path}
        onChange={(e) => setPath(e.target.value)}
      />
      <div className="flex justify-end gap-[var(--spacing-2)]">
        <UIButton variant="ghost" type="button" onClick={onCancel}>
          Cancel
        </UIButton>
        <UIButton variant="primary" type="submit" disabled={!canSubmit}>
          Save changes
        </UIButton>
      </div>
    </form>
  );
}
