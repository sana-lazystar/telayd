/**
 * ChoiceButton unit tests (Group 7 P0)
 * - click → onClick(index) payload
 * - disabled state blocks click
 * - keyboard Enter/Space support
 */

import { fireEvent, render, screen } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import { ChoiceButton } from './ChoiceButton'

describe('ChoiceButton', () => {
  it('renders label and description', () => {
    render(
      <ChoiceButton index={1} label="PostgreSQL" description="Relational DB" onClick={vi.fn()} />,
    )
    expect(screen.getByText('PostgreSQL')).toBeTruthy()
    expect(screen.getByText('Relational DB')).toBeTruthy()
  })

  it('calls onClick with correct index on click', () => {
    const onClick = vi.fn()
    render(<ChoiceButton index={2} label="MySQL" onClick={onClick} />)
    fireEvent.click(screen.getByRole('button'))
    expect(onClick).toHaveBeenCalledWith(2)
  })

  it('does not call onClick when disabled', () => {
    const onClick = vi.fn()
    render(<ChoiceButton index={1} label="SQLite" disabled onClick={onClick} />)
    fireEvent.click(screen.getByRole('button'))
    expect(onClick).not.toHaveBeenCalled()
  })

  it('does not call onClick when loading', () => {
    const onClick = vi.fn()
    render(<ChoiceButton index={1} label="Option" loading onClick={onClick} />)
    fireEvent.click(screen.getByRole('button'))
    expect(onClick).not.toHaveBeenCalled()
  })

  it('calls onClick on Enter key', () => {
    const onClick = vi.fn()
    render(<ChoiceButton index={3} label="Option C" onClick={onClick} />)
    fireEvent.keyDown(screen.getByRole('button'), { key: 'Enter' })
    expect(onClick).toHaveBeenCalledWith(3)
  })

  it('calls onClick on Space key', () => {
    const onClick = vi.fn()
    render(<ChoiceButton index={4} label="Option D" onClick={onClick} />)
    fireEvent.keyDown(screen.getByRole('button'), { key: ' ' })
    expect(onClick).toHaveBeenCalledWith(4)
  })

  it('shows index badge', () => {
    render(<ChoiceButton index={5} label="Option E" onClick={vi.fn()} />)
    expect(screen.getByText('5')).toBeTruthy()
  })
})
