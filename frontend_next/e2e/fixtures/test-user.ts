export const TEST_USER = {
  email: process.env.E2E_TEST_USER_EMAIL || "e2e-test@example.com",
  password: process.env.E2E_TEST_USER_PASSWORD || "E2eTest123!",
  fullName: "E2E Test User",
};

export const COLLAB_USER = {
  email: process.env.E2E_COLLAB_USER_EMAIL || "e2e-collab@test.local",
  password: process.env.E2E_COLLAB_USER_PASSWORD || "E2eCollab123!",
  fullName: "E2E Collab User",
};
