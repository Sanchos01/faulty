# Faulty Server Problem

A server is provided at http://faulty-server-htz-nbg1-1.wvservices.exchange:8080. It is supposed to return a JSON with a single 32-bit integer value. However, it is not very good at its job. It has following restrictions:

- It is somewhat slow
- Sometimes it returns an internal error with status 500
- Occasionally it times out with status 504
- It can handle only a certain number of concurrent connections, returning status 429 if the limit is exceeded

#### Possible faulty server responses

| Status | Response JSON                                 |
| ------ | --------------------------------------------- |
| 200    | `{ "value": 37 }`                             |
| 500    | `{ "error": "Internal server error" }`        |
| 504    | `{ "error": "Timed out" }`                    |
| 429    | `{ "error": "Too many concurrent requests" }` |

## Goal

The goal is to write a server application, providing a JSON API according to a specification below.

### Endpoints

#### Start a run

**Method**: `POST`

**Path**: `/runs`

**Request Body**: `{ "seconds": 30 }`

**Description**: Initiates the process of polling the faulty server called a 'run'. For each run an ID must be generated and returned to a user right away, without waiting for the run finish. The run lifetime must be limited by the `seconds` parameter in POST JSON body. During the run the app should make as many requests to the faulty server as possible. A number of successful requests and a sum of received integer values should be calculated and saved. Only a single concurrent run must be allowed.

**Response**:

```json
{
  "id": "<some generated ID>"
}
```

#### Get run info by ID

**Method**: `GET`

**Path**: `/runs/{id}`

**Description**: Returns run info by the run `id`. The info should include: run status (`IN_PROGRESS` or `FINISHED`), a number of succesful responses received from the faulty server, and a sum of all received integer values.

**Response**:

```json
{
  "status": "IN_PROGRESS",
  "successful_responses_count": 17,
  "sum": 712
}
```
