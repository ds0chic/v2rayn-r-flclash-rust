#include <stdarg.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>

typedef enum OverTlsLogLevel {
  OverTlsLogLevel_Off,
  OverTlsLogLevel_Error,
  OverTlsLogLevel_Warn,
  OverTlsLogLevel_Info,
  OverTlsLogLevel_Debug,
  OverTlsLogLevel_Trace,
} OverTlsLogLevel;

typedef struct OverTlsTrafficStatus {
  uint64_t tx;
  uint64_t rx;
} OverTlsTrafficStatus;

#ifdef __cplusplus
extern "C" {
#endif // __cplusplus

/**
 * # Safety
 *
 * Run the overtls client with config file.
 * The callback function will be called when the client is listening on a port.
 * It should be thread-safe and will be called with the port number and should be called only once.
 * Parameters:
 * - `config_path`: The path to the config file.
 * - `listen_addr`: If not null, it overrides the listen address in the config file. It should be in the format of "ip:port".
 * - `advertise_ip`: The public IP address to be advertised in UDP ASSOCIATE replies. If null, the server will use the local IP address.
 * - `log_level`: The verbosity level of the logger.
 * - `callback`: The callback function to be called when the client is listening on a port. It should be thread-safe and will be called with the port number and should be called only once.
 * - `ctx`: The context pointer to be passed to the callback function.
 *
 */
int over_tls_client_run(const char *config_path,
                        const char *listen_addr,
                        const char *advertise_ip,
                        enum OverTlsLogLevel log_level,
                        void (*callback)(int, void*),
                        void *ctx);

/**
 * # Safety
 *
 * Run the overtls client with SSR URL.
 * Parameters:
 * - `url`: SSR style URL string of the server node, e.g. "ssr://server:port:protocol:method:obfs:password_base64/?params_base64".
 * - `listen_addr`: The address to listen on, in the format of "ip:port".
 * - `advertise_ip`: The public IP address to be advertised in UDP ASSOCIATE replies. If null, the server will use the local IP address.
 * - `log_level`: The verbosity level of the logger.
 * - `callback`: The callback function to be called when the client is listening on a port.
 *   It should be thread-safe and will be called with the port number and should be called only once.
 * - `ctx`: The context pointer to be passed to the callback function.
 *
 */
int over_tls_client_run_with_ssr_url(const char *url,
                                     const char *listen_addr,
                                     const char *advertise_ip,
                                     enum OverTlsLogLevel log_level,
                                     void (*callback)(int, void*),
                                     void *ctx);

/**
 * # Safety
 *
 * Shutdown the client.
 */
int over_tls_client_stop(void);

/**
 * # Safety
 *
 * Create a SSR URL from the config file.
 */
char *overtls_generate_url(const char *cfg_path);

/**
 * # Safety
 *
 * Free the string returned by `overtls_generate_url`.
 */
void overtls_free_string(char *s);

/**
 * # Safety
 *
 * set dump log info callback.
 */
void overtls_set_log_callback(bool set_logger, void (*callback)(enum OverTlsLogLevel,
                                                                const char*,
                                                                void*), void *ctx);

/**
 * # Safety
 *
 * set traffic status callback.
 */
void overtls_set_traffic_status_callback(uint32_t send_interval_secs,
                                         void (*callback)(const struct OverTlsTrafficStatus*, void*),
                                         void *ctx);

#ifdef __cplusplus
}  // extern "C"
#endif  // __cplusplus
