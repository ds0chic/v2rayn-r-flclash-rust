# Synthetic loopback HTTP target for R4-21. Serves a fixed body on any path so
# a proxied request through a freshly installed core proves end-to-end traffic.
# A raw TcpListener avoids the HttpListener URL-ACL requirement; binds only
# 127.0.0.1 on a caller-probed port and is stopped by PID from the caller.
# Every connection is handled defensively so a bare port probe (connect, no
# bytes, close) can never wedge or kill the server.
param(
    [Parameter(Mandatory = $true)][int]$Port,
    [Parameter(Mandatory = $true)][string]$Body
)

$listener = [System.Net.Sockets.TcpListener]::new([System.Net.IPAddress]::Loopback, $Port)
$listener.Start()
$bodyBytes = [System.Text.Encoding]::ASCII.GetBytes($Body)
try {
    while ($true) {
        $client = $listener.AcceptTcpClient()
        try {
            $client.ReceiveTimeout = 2000
            $client.SendTimeout = 2000
            $stream = $client.GetStream()
            $reader = [System.IO.StreamReader]::new($stream, [System.Text.Encoding]::ASCII)
            # Drain request headers; a probe that closes early simply ends here.
            while ($true) {
                $line = $reader.ReadLine()
                if ($null -eq $line -or $line.Length -eq 0) { break }
            }
            $header = "HTTP/1.1 200 OK`r`nContent-Type: text/plain`r`nContent-Length: $($bodyBytes.Length)`r`nConnection: close`r`n`r`n"
            $headerBytes = [System.Text.Encoding]::ASCII.GetBytes($header)
            $stream.Write($headerBytes, 0, $headerBytes.Length)
            $stream.Write($bodyBytes, 0, $bodyBytes.Length)
            $stream.Flush()
        }
        catch {
            # A probe / aborted connection must not stop the server.
        }
        finally {
            try { $client.Close() } catch {}
        }
    }
}
finally {
    $listener.Stop()
}
