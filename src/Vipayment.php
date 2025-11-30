<?php

namespace Vipayment;

/**
 * VIPayment Class
 */
class Vipayment
{
    /**
     * Endpoint API
     * @var string
     */
    public string $endPoint = 'https://vip-reseller.co.id/api';

    /**
     * API ID
     * @var string
     */
    protected string $apiId;

    /**
     * API Key
     * @var string
     */
    protected string $apiKey;

    /**
     * Signature
     * @var string
     */
    protected string $signature;

    /**
     * Constructor
     * 
     * @param string $apiId
     * @param string $apiKey
     */
    public function __construct(string $apiId, string $apiKey)
    {
        $this->apiId = $apiId;
        $this->apiKey = $apiKey;
        $this->signature = md5($apiId . $apiKey);
    }

    /**
     * Get Profile
     * 
     * @return array
     */
    public function profile(): array
    {
        $endPoint = $this->endPoint . '/profile';

        $params = [
            'key' => $this->apiKey,
            'sign' => $this->signature
        ];

        $response = $this->connect($endPoint, $params);

        if (isset($response['result']) && $response['result'] === false) {
            return [
                'status' => false,
                'message' => $response['message'] ?? 'Unknown error'
            ];
        }

        return [
            'status' => true,
            'data' => $response['data'] ?? [],
            'message' => $response['message'] ?? 'Success'
        ];
    }

    /**
     * Order Prepaid
     * 
     * @param string $serviceCode Service code (e.g., 'PLN5', 'PULSA5')
     * @param string $dataNo Target number
     * 
     * @return array
     */
    public function orderPrepaid(string $serviceCode, string $dataNo): array
    {
        $endPoint = $this->endPoint . '/prepaid';

        $params = [
            'key' => $this->apiKey,
            'sign' => $this->signature,
            'type' => 'order',
            'service' => $serviceCode,
            'data_no' => $dataNo
        ];

        $response = $this->connect($endPoint, $params);

        if (isset($response['result']) && $response['result'] === false) {
            return [
                'status' => false,
                'message' => $response['message'] ?? 'Unknown error'
            ];
        }

        return [
            'status' => true,
            'data' => $response['data'] ?? [],
            'message' => $response['message'] ?? 'Success'
        ];
    }

    /**
     * Status Order Prepaid
     * 
     * @param string $trxId Transaction ID
     * @param int|null $limit Limit results (optional)
     * 
     * @return array
     */
    public function statusOrderPrepaid(string $trxId, ?int $limit = null): array
    {
        $endPoint = $this->endPoint . '/prepaid';

        $params = [
            'key' => $this->apiKey,
            'sign' => $this->signature,
            'type' => 'status',
            'trxid' => $trxId,
            'limit' => $limit
        ];

        $response = $this->connect($endPoint, $params);

        if (isset($response['result']) && $response['result'] === false) {
            return [
                'status' => false,
                'message' => $response['message'] ?? 'Unknown error'
            ];
        }

        return [
            'status' => true,
            'data' => $response['data'] ?? [],
            'message' => $response['message'] ?? 'Success'
        ];
    }

    /**
     * Service Prepaid
     * 
     * @param string|null $filterType Filter type (type, brand)
     * @param string|null $filterValue Filter value (pulsa-reguler, telkomsel)
     * 
     * @return array
     */
    public function servicePrepaid(?string $filterType = null, ?string $filterValue = null): array
    {
        $endPoint = $this->endPoint . '/prepaid';

        $params = [
            'key' => $this->apiKey,
            'sign' => $this->signature,
            'type' => 'services',
            'filter_type' => $filterType,
            'filter_value' => $filterValue
        ];

        $response = $this->connect($endPoint, $params);

        if (isset($response['result']) && $response['result'] === false) {
            return [
                'status' => false,
                'message' => $response['message'] ?? 'Unknown error'
            ];
        }

        return [
            'status' => true,
            'data' => $response['data'] ?? [],
            'message' => $response['message'] ?? 'Success'
        ];
    }

    /**
     * Order Game & Streaming
     * 
     * @param string $service Service code
     * @param string $dataNo Target number/ID
     * @param string|null $dataZone Zone ID (optional)
     * 
     * @return array
     */
    public function orderGame(string $service, string $dataNo, ?string $dataZone = null): array
    {
        $endPoint = $this->endPoint . '/game-feature';

        $params = [
            'key' => $this->apiKey,
            'sign' => $this->signature,
            'type' => 'order',
            'service' => $service,
            'data_no' => $dataNo,
            'data_zone' => $dataZone
        ];

        $response = $this->connect($endPoint, $params);

        if (isset($response['result']) && $response['result'] === false) {
            return [
                'status' => false,
                'message' => $response['message'] ?? 'Unknown error'
            ];
        }

        return [
            'status' => true,
            'data' => $response['data'] ?? [],
            'message' => $response['message'] ?? 'Success'
        ];
    }

    /**
     * Status Order Game & Streaming
     * 
     * @param string $trxId Transaction ID
     * @param int|null $limit Limit results (optional)
     * 
     * @return array
     */
    public function statusOrderGame(string $trxId, ?int $limit = null): array
    {
        $endPoint = $this->endPoint . '/game-feature';

        $params = [
            'key' => $this->apiKey,
            'sign' => $this->signature,
            'type' => 'status',
            'trxid' => $trxId,
            'limit' => $limit
        ];

        $response = $this->connect($endPoint, $params);

        if (isset($response['result']) && $response['result'] === false) {
            return [
                'status' => false,
                'message' => $response['message'] ?? 'Unknown error'
            ];
        }

        return [
            'status' => true,
            'data' => $response['data'] ?? [],
            'message' => $response['message'] ?? 'Success'
        ];
    }

    /**
     * Service Game & Streaming
     * 
     * @param string|null $filterGame Filter game (Mobile Legends A, CapCut Pro)
     * @param string|null $filterStatus Filter status (available, empty)
     * 
     * @return array
     */
    public function serviceGame(?string $filterGame = null, ?string $filterStatus = null): array
    {
        $endPoint = $this->endPoint . '/game-feature';

        $params = [
            'key' => $this->apiKey,
            'sign' => $this->signature,
            'type' => 'services',
            'filter_game' => $filterGame,
            'filter_status' => $filterStatus
        ];

        $response = $this->connect($endPoint, $params);

        if (isset($response['result']) && $response['result'] === false) {
            return [
                'status' => false,
                'message' => $response['message'] ?? 'Unknown error'
            ];
        }

        return [
            'status' => true,
            'data' => $response['data'] ?? [],
            'message' => $response['message'] ?? 'Success'
        ];
    }

    /**
     * Connect to API
     * 
     * @param string $endpoint
     * @param array $params
     * 
     * @return array|bool
     */
    private function connect(string $endpoint, array $params): array|bool
    {
        $ch = curl_init($endpoint);
        curl_setopt($ch, CURLOPT_RETURNTRANSFER, true);
        curl_setopt($ch, CURLOPT_POST, true);
        curl_setopt($ch, CURLOPT_HEADER, false);
        curl_setopt($ch, CURLOPT_SSL_VERIFYPEER, false);
        curl_setopt($ch, CURLOPT_SSL_VERIFYHOST, false);
        curl_setopt($ch, CURLOPT_FOLLOWLOCATION, true);
        
        if (!empty($params)) {
            curl_setopt($ch, CURLOPT_POSTFIELDS, http_build_query($params));
        }
        
        curl_setopt($ch, CURLOPT_USERAGENT, 'VIPayment-PHP-SDK/1.0.0');
        
        $response = curl_exec($ch);
        $error = curl_error($ch);
        $errno = curl_errno($ch);
        
        curl_close($ch);

        if ($errno !== 0) {
            return [
                'result' => false,
                'message' => 'cURL Error: ' . $error
            ];
        }

        if (empty($response)) {
            return [
                'result' => false,
                'message' => 'Empty response from server'
            ];
        }

        return json_decode($response, true) ?? [
            'result' => false,
            'message' => 'Invalid JSON response'
        ];
    }
}
