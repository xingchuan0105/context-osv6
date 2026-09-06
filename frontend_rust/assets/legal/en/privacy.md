---
title: Privacy Policy
slug: privacy
version: "2026-06-13"
locale: en
status: published
content_owner: frontend
legal_review: approved
last_legal_review_at: 2026-06-13
last_legal_reviewer: "Xing Chuan (legal + product)"
next_review_due: 2026-09-13
---

# Privacy Policy

Last updated: 2026-06-13 · Version: 2026-06-13

> This English translation is provided for convenience only. In case of any discrepancy, the Chinese version shall prevail.

Context-OS ("we," "us," or the "Service") takes the protection of your personal information very seriously. This Privacy Policy explains in detail how we collect, use, store, and protect your personal information. Please read this Privacy Policy carefully before using the Service.

## 1. Information We Collect

### 1.1 Account Identification Information

- **Information collected:** Email address and display name (optional)
- **Purpose of processing:** Account registration, identity authentication, password recovery, and Service notifications
- **Storage method:** Stored in encrypted form in a PostgreSQL database
- **Retention period:** For the life of your account; following account deletion, information will be deleted after any retention period required by law

### 1.2 Uploaded Documents and Metadata

- **Information collected:** Document files you upload (PDF, text, and other formats) and metadata such as file names, upload times, and file sizes
- **Purpose of processing:** RAG document processing, text extraction, vector indexing, retrieval, and question answering
- **Storage method:** Files are stored in object storage (cloud S3/OSS in production); metadata and parsing results are stored in PostgreSQL
- **Retention period:** For the life of your account; you may delete uploaded documents at any time
- **Use for model training:** **No.** Documents you upload will not be used to train, fine-tune, or improve AI models

### 1.3 Chat and Reasoning Content

- **Information collected:** Records of your conversations with AI, your queries, and AI-generated responses
- **Purpose of processing:** Providing conversational services, contextual memory (if enabled), and Service quality improvement
- **Storage method:** Stored in a PostgreSQL database
- **Retention period:** For the life of your account; you may delete conversation records in the application at any time
- **Export method:** You may export your conversation history through the Settings page

### 1.4 Vector Embeddings

- **Information collected:** Mathematical vector representations generated from document content
- **Purpose of processing:** Enabling semantic retrieval and similarity search
- **Storage method:** Stored in the Milvus vector database
- **Technical information:** Vector embeddings are numerical representations of document content and do not contain directly identifiable personal information. When an original document is deleted, the corresponding vector data will also be deleted.

### 1.5 Usage and Billing Information

- **Information collected:** Number of API calls, storage usage, feature usage, and subscription status
- **Purpose of processing:** Quota management, billing calculations, and Service optimization
- **Storage method:** Stored in a PostgreSQL database
- **Third-party interactions:** Payment information is processed by Creem or Alipay. We receive only the payment status (successful or failed) and necessary transaction identifiers and do not store your complete payment card information

### 1.6 Third-Party AI/OCR Services

To provide core features, we transmit certain data to the following third-party service providers:

| Service provider | Purpose | Data type |
|------------------|---------|-----------|
| DeepSeek | AI inference | Conversation content and document context |
| DashScope (Alibaba Cloud Model Studio) | Text embeddings and multimodal processing | Document text and query content |
| Paddle OCR | Document text recognition | Document images |
| Brave Search | Web retrieval augmentation | Search queries (only when web search is enabled) |

- **Entrusted processing:** These data transfers are limited to what is necessary to provide the Service to you
- **Cross-border transfer:** Some service providers may operate servers outside mainland China. We will ensure that data transfers comply with applicable data protection laws
- **Security safeguards:** We use these services through API calls, and data is transmitted using encrypted protocols

### 1.7 Log and Audit Information

- **Information collected:** Access logs (IP address, access time, and request path), error logs, and security audit events
- **Purpose of processing:** Service operation and maintenance, troubleshooting, security protection, and compliance auditing
- **Storage method:** Server log files
- **Retention period:** Automatically rotated and deleted after 30 days; security audit logs are retained for as long as necessary

### 1.8 Device and Browser Information

- **Information collected:** Browser type, operating system, screen resolution, and language preferences
- **Purpose of processing:** Optimizing interface compatibility and troubleshooting compatibility issues
- **Storage method:** Client-side cookies or localStorage (not used for tracking)

## 2. Principles Governing Our Use of Information

We process your personal information in accordance with the following principles:

- **Lawfulness and propriety:** We collect and use personal information only to the extent necessary to provide the Service to you
- **Data minimization:** We do not collect personal information unrelated to the Service
- **Purpose limitation:** We do not use your information for purposes not described in this Privacy Policy
- **Security safeguards:** We take reasonable technical and organizational measures to protect information security

## 3. Information Sharing and Disclosure

### 3.1 No Sale of Personal Information

We do not sell your personal information.

### 3.2 Circumstances in Which We Share Information

We share your information with third parties only in the following circumstances:

- **Service providers:** Technical partners necessary to provide the Service (see §1.6)
- **Legal requirements:** As required by laws, regulations, legal proceedings, or competent government authorities
- **Security protection:** As necessary to protect our rights, property, or safety, or those of you or the public
- **Business changes:** In connection with a merger, acquisition, or sale of assets, information may be transferred as a business asset (we will notify you in advance)

### 3.3 Anonymized Data

We may use information that has been anonymized or aggregated for statistical analysis, product improvement, and other purposes. Anonymized data can no longer be used to identify a specific individual.

## 4. Data Security

### 4.1 Technical Measures

- Encryption in transit (HTTPS/TLS)
- Database access controls and encryption
- API authentication and authorization mechanisms
- Regular security audits and vulnerability scans

### 4.2 Organizational Measures

- Principle of least privilege: employees may access only the data necessary for their work
- Security awareness training
- Security incident response procedures

### 4.3 Security Incidents

If a personal information security incident occurs, we will promptly notify affected users as required by applicable laws and regulations and take remedial measures.

## 5. Your Rights

Under applicable personal information protection laws, including, without limitation, the Personal Information Protection Law of the People's Republic of China (《个人信息保护法》; "PIPL") and the Data Security Law of the People's Republic of China (《数据安全法》), you have the following rights:

### 5.1 Right of Access

You have the right to access the personal information we have collected about you.

### 5.2 Right to Obtain a Copy

You have the right to obtain a copy of your personal information.

### 5.3 Right to Correction

You have the right to request correction of inaccurate personal information.

### 5.4 Right to Deletion

You have the right to request deletion of your personal information if:

- The purposes of processing have been achieved or the information is no longer necessary
- You withdraw your consent
- We process the information in violation of applicable law

### 5.5 Account Deletion

You may delete your account in either of the following ways:

- **Self-service deletion:** Submit an account deletion request through the Settings page
- **Contact customer support:** Email [legal@context-os.com](mailto:legal@context-os.com)

After account deletion, we will delete or anonymize your personal information within a reasonable period, unless otherwise required by applicable laws or regulations.

### 5.6 Withdrawal of Consent

Where the processing of personal information is based on consent, you have the right to withdraw your consent at any time. Withdrawal of consent does not affect the lawfulness of processing carried out before the withdrawal.

### 5.7 How to Exercise Your Rights

You may exercise the rights described above through the following channels:

- **In-app controls:** Some rights may be exercised directly through the Settings page
- **Email:** Send your request to [legal@context-os.com](mailto:legal@context-os.com)
- **Response time:** We will respond to your request within 15 business days

## 6. Cookies and Local Storage

The Service uses necessary cookies and localStorage to provide the following functions:

- **Session management:** Maintaining your logged-in state
- **Preferences:** Remembering your interface language, theme, and other preferences
- **Security protection:** Security mechanisms such as CSRF protection

We do not use third-party tracking cookies. You may manage or clear local storage through your browser settings.

## 7. Protection of Minors

The Service is not intended for minors under the age of 14. If you are a minor, please use the Service under the guidance of your guardian. If we discover that we have collected a minor's personal information, we will delete it as soon as practicable after verification.

## 8. Updates to This Privacy Policy

### 8.1 How We Update This Privacy Policy

- **General changes:** We will update this page and revise the "Last updated" date
- **Material changes:** We will notify you in advance by email, in-product notice, or other means

### 8.2 Your Choices

By continuing to use the Service after this Privacy Policy is updated, you agree to the updated Privacy Policy. If you do not agree to the updated Privacy Policy, you should stop using the Service and delete your account.

## 9. Contact Us

If you have any questions, comments, or requests concerning this Privacy Policy, please contact us through the following channel:

- **Email:** [legal@context-os.com](mailto:legal@context-os.com)
- **Response time:** Within 15 business days

---

This Privacy Policy forms part of the Context-OS Terms of Service.
