import type { FieldInput } from "../types/field";

export interface TemplateTable { name: string; fields: FieldInput[] }
export interface Template { id: string; name: string; description: string; icon: string; tables: TemplateTable[] }

function sel(opts: string[]): FieldInput["config"] { return { options: opts.map((name, i) => ({ id: `opt_${i}`, name, color: ["#6d7bff","#ff6b9d","#4ecdc4","#feca57","#a55eea","#26de81"][i%6] })) } as unknown as Record<string, unknown>; }

export const TEMPLATES: Template[] = [
  {
    id: "crm",
    name: "CRM Simple",
    description: "Contacts, Opportunités, Interactions — suivi commercial",
    icon: "👥",
    tables: [
      {
        name: "Contacts",
        fields: [
          { name: "Nom", type: "text", config: {} },
          { name: "Email", type: "email", config: {} },
          { name: "Société", type: "text", config: {} },
          { name: "Téléphone", type: "phone", config: {} },
          { name: "Statut", type: "select", config: sel(["Prospect","Client","Perdu"]) },
          { name: "Valeur estimée", type: "number", config: {} },
          { name: "Dernière interaction", type: "date", config: {} },
        ],
      },
      {
        name: "Opportunités",
        fields: [
          { name: "Nom", type: "text", config: {} },
          { name: "Montant", type: "number", config: {} },
          { name: "Étape", type: "select", config: sel(["Découverte","Proposition","Négociation","Gagnée","Perdue"]) },
          { name: "Probabilité %", type: "number", config: {} },
          { name: "Échéance", type: "date", config: {} },
          { name: "Notes", type: "long_text", config: {} },
        ],
      },
      {
        name: "Interactions",
        fields: [
          { name: "Date", type: "date", config: {} },
          { name: "Type", type: "select", config: sel(["Appel","Email","Réunion","Note"]) },
          { name: "Notes", type: "long_text", config: {} },
          { name: "Fait", type: "checkbox", config: {} },
        ],
      },
    ],
  },
  {
    id: "facturation",
    name: "Facturation",
    description: "Clients, Factures, Lignes — devis & suivi paiement",
    icon: "🧾",
    tables: [
      {
        name: "Clients",
        fields: [
          { name: "Nom", type: "text", config: {} },
          { name: "Email", type: "email", config: {} },
          { name: "Adresse", type: "long_text", config: {} },
          { name: "Siret", type: "text", config: {} },
        ],
      },
      {
        name: "Factures",
        fields: [
          { name: "Numéro", type: "text", config: {} },
          { name: "Date", type: "date", config: {} },
          { name: "Statut", type: "select", config: sel(["Brouillon","Envoyée","Payée","En retard"]) },
          { name: "Montant HT", type: "number", config: {} },
          { name: "Payée", type: "checkbox", config: {} },
          { name: "Notes", type: "long_text", config: {} },
        ],
      },
      {
        name: "Lignes",
        fields: [
          { name: "Description", type: "text", config: {} },
          { name: "Quantité", type: "number", config: {} },
          { name: "Prix unitaire", type: "number", config: {} },
          { name: "Montant", type: "number", config: {} },
        ],
      },
    ],
  },
  {
    id: "projets",
    name: "Projets",
    description: "Projets, Tâches, Équipe — suivi d'avancement",
    icon: "🚀",
    tables: [
      {
        name: "Projets",
        fields: [
          { name: "Nom", type: "text", config: {} },
          { name: "Statut", type: "select", config: sel(["Idée","En cours","En revue","Terminé"]) },
          { name: "Chef", type: "text", config: {} },
          { name: "Début", type: "date", config: {} },
          { name: "Échéance", type: "date", config: {} },
        ],
      },
      {
        name: "Tâches",
        fields: [
          { name: "Titre", type: "text", config: {} },
          { name: "Statut", type: "select", config: sel(["À faire","En cours","Bloqué","Fait"]) },
          { name: "Priorité", type: "select", config: sel(["Basse","Moyenne","Haute","Urgente"]) },
          { name: "Assigné à", type: "text", config: {} },
          { name: "Échéance", type: "date", config: {} },
          { name: "Fait", type: "checkbox", config: {} },
        ],
      },
      {
        name: "Équipe",
        fields: [
          { name: "Nom", type: "text", config: {} },
          { name: "Rôle", type: "select", config: sel(["Dev","Design","Produit","Marketing"]) },
          { name: "Email", type: "email", config: {} },
          { name: "Dispo", type: "checkbox", config: {} },
        ],
      },
    ],
  },
];
