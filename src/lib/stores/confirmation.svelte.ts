/** A pending question for the confirmation dialog, if any. */
export interface Question {
  title: string;
  message: string;
  confirmLabel: string;
  resolve: (confirmed: boolean) => void;
}

export const confirmation: { question: Question | null } = $state({
  question: null,
});

/** Shows the confirmation dialog; resolves to whether the user confirmed. */
export function askToConfirm(
  title: string,
  message: string,
  confirmLabel: string,
): Promise<boolean> {
  confirmation.question?.resolve(false);
  return new Promise((resolve) => {
    confirmation.question = { title, message, confirmLabel, resolve };
  });
}

export function answer(confirmed: boolean): void {
  const question = confirmation.question;
  confirmation.question = null;
  question?.resolve(confirmed);
}
