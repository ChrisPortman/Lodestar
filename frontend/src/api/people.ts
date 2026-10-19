import { ApiError, client, ifMatch, unwrap, type Person, type Session } from './client'

export async function listPeople(): Promise<Person[]> {
  const page = unwrap(
    await client.GET('/api/v1/people', {
      params: { query: { sort: 'display_name', limit: 100 } },
    }),
  )
  return page.items
}

/** Creates a Person; the server also makes them the Acting Person. */
export async function createPerson(displayName: string): Promise<Person> {
  return unwrap(
    await client.POST('/api/v1/people', {
      body: { display_name: displayName } as Person,
    }),
  )
}

export async function renamePerson(person: Person, displayName: string): Promise<Person> {
  return unwrap(
    await client.PUT('/api/v1/people/{id}', {
      params: { path: { id: person.id }, header: { 'if-match': ifMatch(person.version) } },
      body: { ...person, display_name: displayName },
    }),
  )
}

/** The current session, or `null` when there is no Acting Person. */
export async function getSession(): Promise<Session | null> {
  try {
    return unwrap(await client.GET('/api/v1/session'))
  } catch (e) {
    if (e instanceof ApiError && e.status === 401) return null
    throw e
  }
}

export async function startSession(personId: string): Promise<Session> {
  return unwrap(await client.POST('/api/v1/session', { body: { person_id: personId } }))
}
