# Synthetic Office files for real browser acceptance

`inventory.xlsx` contains a Cedar row with 137 units. `delivery.pptx` contains
the delivery label Violet Harbor. These are small, valid Office packages made
with openpyxl and python-pptx, for exercising actual backend parsing. They do
not contain customer data and must not be copied into product prompts.

The opt-in `real-journey.spec.ts` uses these files without mocking parser or
model responses. The regular fixture suite excludes that spec.
