import { useState } from "react";
import { UIInput } from "@/components/ui/UIInput";
import { UISelect } from "@/components/ui/UISelect";
import { UIButton } from "@/components/ui/UIButton";
import { UIText } from "@/components/ui/UIText";

interface AddPortFormProps {
  rangeStart: number;
  rangeEnd: number;
  usedPorts: number[];
  onSubmit: (service: string, port: number) => void;
  onCancel: () => void;
}

export function AddPortForm({
  rangeStart,
  rangeEnd,
  usedPorts,
  onSubmit,
  onCancel,
}: AddPortFormProps) {
  const availablePorts = [];
  for (let p = rangeStart; p <= rangeEnd; p++) {
    if (!usedPorts.includes(p)) availablePorts.push(p);
  }

  const [service, setService] = useState("");
  const [port, setPort] = useState(availablePorts[0]?.toString() ?? "");

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    const portNum = parseInt(port, 10);
    if (service.trim() && !isNaN(portNum)) {
      onSubmit(service.trim(), portNum);
    }
  };

  if (availablePorts.length === 0) {
    return (
      <div className="bg-bg-elevated rounded-[var(--radius-md)] p-[var(--spacing-3)]">
        <UIText variant="body" className="text-text-muted">
          All ports in the range are taken
        </UIText>
      </div>
    );
  }

  return (
    <form
      onSubmit={handleSubmit}
      className="flex flex-col gap-[var(--spacing-2)] bg-bg-elevated rounded-[var(--radius-md)] p-[var(--spacing-3)]"
    >
      <div className="flex gap-[var(--spacing-2)] items-end">
        <UIInput
          label="Service"
          placeholder="e.g. vite, postgres"
          value={service}
          onChange={(e) => setService(e.target.value)}
          wrapperClassName="w-48"
          autoFocus
        />
        <UISelect
          label="Port"
          value={port}
          onChange={setPort}
          className="w-24"
          options={availablePorts.map((p) => ({
            value: String(p),
            label: String(p),
          }))}
        />
      </div>
      <div className="flex justify-end gap-[var(--spacing-2)]">
        <UIButton variant="ghost" type="button" onClick={onCancel}>
          Cancel
        </UIButton>
        <UIButton
          variant="primary"
          type="submit"
          disabled={!service.trim()}
        >
          Add
        </UIButton>
      </div>
    </form>
  );
}
