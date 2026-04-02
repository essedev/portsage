import { useState } from "react";
import { GrimInput } from "@/components/ui/GrimInput";
import { GrimSelect } from "@/components/ui/GrimSelect";
import { GrimButton } from "@/components/ui/GrimButton";
import { GrimText } from "@/components/ui/GrimText";

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
        <GrimText variant="body" className="text-text-muted">
          Tutte le porte del range sono occupate
        </GrimText>
      </div>
    );
  }

  return (
    <form
      onSubmit={handleSubmit}
      className="flex flex-col gap-[var(--spacing-2)] bg-bg-elevated rounded-[var(--radius-md)] p-[var(--spacing-3)]"
    >
      <div className="flex gap-[var(--spacing-2)] items-end">
        <GrimInput
          label="Servizio"
          placeholder="es. vite, postgres"
          value={service}
          onChange={(e) => setService(e.target.value)}
          wrapperClassName="w-48"
          autoFocus
        />
        <GrimSelect
          label="Porta"
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
        <GrimButton variant="ghost" type="button" onClick={onCancel}>
          Annulla
        </GrimButton>
        <GrimButton
          variant="primary"
          type="submit"
          disabled={!service.trim()}
        >
          Aggiungi
        </GrimButton>
      </div>
    </form>
  );
}
