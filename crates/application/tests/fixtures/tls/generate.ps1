# Generates the deterministic TLS fixtures used by UX-TEST-02 HTTPS probe tests.
#   ca.pem         self-signed CA (BasicConstraints CA:TRUE, keyCertSign)
#   server.pem     leaf certificate for DNS:localhost + IP:127.0.0.1
#   server.key.pem leaf PKCS#8 private key
# Run from this directory: pwsh -File generate.ps1
# These are test-only synthetic material; they never leave the test tree.
$ErrorActionPreference = 'Stop'
$here = Split-Path -Parent $MyInvocation.MyCommand.Path

function New-Rsa { [System.Security.Cryptography.RSA]::Create(2048) }

# --- CA ---
$caKey = New-Rsa
$caReq = [System.Security.Cryptography.X509Certificates.CertificateRequest]::new(
  'CN=UX-TEST-02 Test CA',
  $caKey,
  [System.Security.Cryptography.HashAlgorithmName]::SHA256,
  [System.Security.Cryptography.RSASignaturePadding]::Pkcs1)
$caReq.CertificateExtensions.Add(
  [System.Security.Cryptography.X509Certificates.X509BasicConstraintsExtension]::new($true, $false, 0, $true))
$caReq.CertificateExtensions.Add(
  [System.Security.Cryptography.X509Certificates.X509KeyUsageExtension]::new(
    [System.Security.Cryptography.X509Certificates.X509KeyUsageFlags]::KeyCertSign -bor
    [System.Security.Cryptography.X509Certificates.X509KeyUsageFlags]::CrlSign, $true))
$caReq.CertificateExtensions.Add(
  [System.Security.Cryptography.X509Certificates.X509SubjectKeyIdentifierExtension]::new($caReq.PublicKey, $false))
$caNotBefore = [DateTimeOffset]::UtcNow.AddDays(-1)
$caNotAfter = [DateTimeOffset]::UtcNow.AddYears(9)
$caCert = $caReq.CreateSelfSigned($caNotBefore, $caNotAfter)

# --- Leaf ---
$leafKey = New-Rsa
$leafReq = [System.Security.Cryptography.X509Certificates.CertificateRequest]::new(
  'CN=localhost',
  $leafKey,
  [System.Security.Cryptography.HashAlgorithmName]::SHA256,
  [System.Security.Cryptography.RSASignaturePadding]::Pkcs1)
$leafReq.CertificateExtensions.Add(
  [System.Security.Cryptography.X509Certificates.X509BasicConstraintsExtension]::new($false, $false, 0, $true))
$leafReq.CertificateExtensions.Add(
  [System.Security.Cryptography.X509Certificates.X509KeyUsageExtension]::new(
    [System.Security.Cryptography.X509Certificates.X509KeyUsageFlags]::DigitalSignature -bor
    [System.Security.Cryptography.X509Certificates.X509KeyUsageFlags]::KeyEncipherment, $true))
$san = [System.Security.Cryptography.X509Certificates.SubjectAlternativeNameBuilder]::new()
$san.AddDnsName('localhost')
$san.AddIpAddress([System.Net.IPAddress]::Loopback)
$leafReq.CertificateExtensions.Add($san.Build())
$leafReq.CertificateExtensions.Add(
  [System.Security.Cryptography.X509Certificates.X509SubjectKeyIdentifierExtension]::new($leafReq.PublicKey, $false))
$leafSerial = [byte[]](1..16 | ForEach-Object { Get-Random -Minimum 1 -Maximum 255 })
$leafCert = $leafReq.Create($caCert, $caNotBefore, [DateTimeOffset]::UtcNow.AddYears(2), $leafSerial)

$caPem = $caCert.ExportCertificatePem()
$leafPem = $leafCert.ExportCertificatePem()
$leafKeyPem = $leafKey.ExportPkcs8PrivateKeyPem()

Set-Content -LiteralPath (Join-Path $here 'ca.pem') -Value $caPem -NoNewline
Set-Content -LiteralPath (Join-Path $here 'server.pem') -Value $leafPem -NoNewline
Set-Content -LiteralPath (Join-Path $here 'server.key.pem') -Value $leafKeyPem -NoNewline
Write-Output "wrote ca.pem server.pem server.key.pem to $here"
