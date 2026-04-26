const RESET_EMAIL_STORAGE_KEY = "context_os.reset.email.v1";
const RESET_TICKET_STORAGE_KEY = "context_os.reset.ticket.v1";

function readStorageValue(key: string) {
  if (typeof window === "undefined") {
    return null;
  }

  return window.sessionStorage.getItem(key);
}

function writeStorageValue(key: string, value: string) {
  if (typeof window === "undefined") {
    return;
  }

  window.sessionStorage.setItem(key, value);
}

function removeStorageValue(key: string) {
  if (typeof window === "undefined") {
    return;
  }

  window.sessionStorage.removeItem(key);
}

export function storeResetEmail(email: string) {
  writeStorageValue(RESET_EMAIL_STORAGE_KEY, email);
}

export function readResetEmail() {
  return readStorageValue(RESET_EMAIL_STORAGE_KEY);
}

export function clearResetEmail() {
  removeStorageValue(RESET_EMAIL_STORAGE_KEY);
}

export function storeResetTicket(ticket: string) {
  writeStorageValue(RESET_TICKET_STORAGE_KEY, ticket);
}

export function readResetTicket() {
  return readStorageValue(RESET_TICKET_STORAGE_KEY);
}

export function clearResetTicket() {
  removeStorageValue(RESET_TICKET_STORAGE_KEY);
}

export function clearResetFlowState() {
  clearResetEmail();
  clearResetTicket();
}
