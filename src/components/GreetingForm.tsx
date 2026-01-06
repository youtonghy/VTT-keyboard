import { useTranslation } from "react-i18next";

type GreetingFormProps = {
  name: string;
  onNameChange: (name: string) => void;
  onSubmit: () => void;
};

export function GreetingForm({
  name,
  onNameChange,
  onSubmit,
}: GreetingFormProps) {
  const { t } = useTranslation();

  return (
    <form
      className="row"
      onSubmit={(event) => {
        event.preventDefault();
        onSubmit();
      }}
    >
      <input
        id="greet-input"
        value={name}
        onChange={(event) => onNameChange(event.currentTarget.value)}
        placeholder={t("greeting.placeholder")}
      />
      <button type="submit">{t("greeting.button")}</button>
    </form>
  );
}
